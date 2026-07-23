import assert from "node:assert/strict"
import { test } from "node:test"

import { GatewayApiError, readSse } from "../lib/gateway-api.ts"

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
