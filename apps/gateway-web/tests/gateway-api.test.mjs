import assert from "node:assert/strict"
import { test } from "node:test"

import { GatewayApiError, readSse } from "../lib/gateway-api.ts"
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
