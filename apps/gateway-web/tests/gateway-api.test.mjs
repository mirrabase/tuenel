import assert from "node:assert/strict"
import { test } from "node:test"

import {
  GatewayApiError,
  gatewayResponse,
  readSse,
} from "../lib/gateway-api.ts"
import { hasValidOrigin } from "../lib/request-origin.ts"

test("SSE reader handles split chunks and done sentinel", async () => {
  const encoder = new TextEncoder()
  const stream = new ReadableStream({
    start(controller) {
      controller.enqueue(encoder.encode('data: {"delta":"a"}\n'))
      controller.enqueue(
        encoder.encode('\ndata: {"delta":"b"}\r\n\r\ndata: [DONE]\n\n')
      )
      controller.close()
    },
  })
  const events = []
  await readSse(new Response(stream), (event) => events.push(event))
  assert.deepEqual(events, [{ delta: "a" }, { delta: "b" }])
})

test("gateway errors retain safe status and request correlation", () => {
  const error = new GatewayApiError("Conflict", 409, "conflict", "request-1")
  assert.equal(error.status, 409)
  assert.equal(error.code, "conflict")
  assert.equal(error.requestId, "request-1")
})

test("gateway requests never infer project scope from the browser URL", async () => {
  const previousWindow = globalThis.window
  const previousFetch = globalThis.fetch
  let requested
  globalThis.window = {
    location: {
      origin: "http://localhost:3000",
      pathname:
        "/en/01900000-0000-7000-8000-000000000001/project/01900000-0000-7000-8000-000000000002/models",
    },
  }
  globalThis.fetch = async (input) => {
    requested = new URL(input)
    return new Response("{}", { status: 200 })
  }
  await gatewayResponse(
    "/admin/projects?limit=10",
    "01900000-0000-7000-8000-000000000001"
  )
  assert.equal(requested.searchParams.get("project_id"), null)
  globalThis.window = previousWindow
  globalThis.fetch = previousFetch
})

test("gateway requests forward an explicit project scope", async () => {
  const previousWindow = globalThis.window
  const previousFetch = globalThis.fetch
  let requested
  globalThis.window = { location: { origin: "http://localhost:3000" } }
  globalThis.fetch = async (_input, init) => {
    requested = new Headers(init.headers)
    return new Response("{}", { status: 200 })
  }
  await gatewayResponse(
    "/v1/models",
    "01900000-0000-7000-8000-000000000001",
    undefined,
    "01900000-0000-7000-8000-000000000002"
  )
  assert.equal(
    requested.get("x-tuenel-project-id"),
    "01900000-0000-7000-8000-000000000002"
  )
  globalThis.window = previousWindow
  globalThis.fetch = previousFetch
})

test("origin validation uses the public forwarded origin", () => {
  const request = (origin, forwardedHost = "console.example.com") =>
    new Request("http://0.0.0.0:3000/api/gateway/admin/virtual-keys", {
      method: "POST",
      headers: {
        host: "0.0.0.0:3000",
        origin,
        "x-forwarded-host": forwardedHost,
        "x-forwarded-proto": "https",
      },
    })

  assert.equal(hasValidOrigin(request("https://console.example.com")), true)
  assert.equal(hasValidOrigin(request("https://attacker.example.com")), false)
  assert.equal(hasValidOrigin(request("http://console.example.com")), false)
  assert.equal(hasValidOrigin(request("not a url")), false)
  assert.equal(
    hasValidOrigin(
      new Request("http://0.0.0.0:3000/api/gateway/admin/virtual-keys", {
        method: "POST",
        headers: { host: "localhost:3000" },
      })
    ),
    false
  )
})
