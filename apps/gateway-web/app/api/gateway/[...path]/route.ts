import { gatewayApiUrl, sessionCredential } from "@/lib/server-auth"

const rules: [RegExp, ReadonlySet<string>][] = [
  [/^\/(health|ready|openapi\.json)$/, new Set(["GET"])],
  [
    /^\/auth\/(tenants|invitations)(\/|$)/,
    new Set(["GET", "POST", "PATCH", "DELETE"]),
  ],
  [
    /^\/v1\/(models|chat\/completions|responses|embeddings)$/,
    new Set(["GET", "POST"]),
  ],
  [/^\/v1\/(mcp|gateway\/approvals)\//, new Set(["GET", "POST"])],
  [/^\/admin\//, new Set(["GET", "POST", "PATCH", "DELETE"])],
]

async function forward(
  request: Request,
  { params }: { params: Promise<{ path: string[] }> }
) {
  const incoming = new URL(request.url)
  const path = `/${(await params).path.join("/")}`
  const allowed = rules.some(
    ([pattern, methods]) => pattern.test(path) && methods.has(request.method)
  )
  if (!allowed)
    return Response.json(
      { error: { code: "not_found", message: "Gateway path is not allowed" } },
      { status: 404 }
    )
  if (
    !["GET", "HEAD"].includes(request.method) &&
    request.headers.get("origin") !== incoming.origin
  )
    return Response.json(
      { error: { code: "invalid_origin", message: "Invalid request origin" } },
      { status: 403 }
    )

  const credential = await sessionCredential()
  const tenant = incoming.searchParams.get("tenant")
  if (!credential || !tenant || !/^[0-9a-f-]{36}$/i.test(tenant))
    return Response.json(
      { error: { code: "unauthorized", message: "Authentication required" } },
      { status: 401 }
    )

  incoming.searchParams.delete("tenant")
  const headers = new Headers()
  for (const name of [
    "accept",
    "content-type",
    "if-match",
    "idempotency-key",
  ]) {
    const value = request.headers.get(name)
    if (value) headers.set(name, value)
  }
  headers.set("authorization", `Bearer ${credential}.${tenant}`)
  try {
    const upstream = await fetch(`${gatewayApiUrl(path)}${incoming.search}`, {
      method: request.method,
      headers,
      body: ["GET", "HEAD"].includes(request.method) ? null : request.body,
      duplex: "half",
      signal: request.signal,
      cache: "no-store",
    } as RequestInit)
    if (upstream.status >= 500) {
      upstream.body?.cancel()
      return Response.json(
        {
          error: {
            code: "upstream_unavailable",
            message: "Gateway service is unavailable",
          },
        },
        { status: upstream.status }
      )
    }
    const responseHeaders = new Headers()
    for (const name of ["content-type", "etag", "x-request-id"]) {
      const value = upstream.headers.get(name)
      if (value) responseHeaders.set(name, value)
    }
    return new Response(upstream.body, {
      status: upstream.status,
      headers: responseHeaders,
    })
  } catch {
    if (request.signal.aborted)
      return new Response(null, {
        status: 499,
        statusText: "Client Closed Request",
      })
    console.error("gateway upstream request failed")
    return Response.json(
      {
        error: {
          code: "upstream_unavailable",
          message: "Gateway service is unavailable",
        },
      },
      { status: 502 }
    )
  }
}

export const GET = forward
export const POST = forward
export const PATCH = forward
export const DELETE = forward
