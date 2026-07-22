import { gatewayApiUrl, sessionCredential } from "@/lib/server-auth"

const allowed = [
  /^\/health$/,
  /^\/ready$/,
  /^\/openapi\.json$/,
  /^\/metrics$/,
  /^\/auth\/(tenants|invitations)(\/|$)/,
  /^\/v1\/(models|chat\/completions|responses|embeddings)$/,
  /^\/v1\/mcp\//,
  /^\/v1\/gateway\/approvals\//,
  /^\/admin\/(virtual-keys|mcp|approvals|security)(\/|$)/,
]

async function forward(
  request: Request,
  { params }: { params: Promise<{ path: string[] }> }
) {
  const url = new URL(request.url)
  const path = `/${(await params).path.join("/")}`
  if (!allowed.some((pattern) => pattern.test(path)))
    return Response.json(
      { error: "Gateway path is not allowed" },
      { status: 404 }
    )
  if (
    request.method !== "GET" &&
    request.method !== "HEAD" &&
    request.headers.get("origin") !== url.origin
  )
    return Response.json({ error: "Invalid request origin" }, { status: 403 })

  const credential = await sessionCredential()
  const tenant = url.searchParams.get("tenant")
  if (!credential || !tenant || !/^[0-9a-f-]{36}$/i.test(tenant))
    return Response.json({ error: "Authentication required" }, { status: 401 })

  url.searchParams.delete("tenant")
  const headers = new Headers(request.headers)
  headers.delete("cookie")
  headers.delete("host")
  headers.delete("authorization")
  headers.set("authorization", `Bearer ${credential}.${tenant}`)
  const upstream = await fetch(`${gatewayApiUrl(path)}${url.search}`, {
    method: request.method,
    headers,
    body:
      request.method === "GET" || request.method === "HEAD"
        ? null
        : request.body,
    duplex: "half",
  } as RequestInit)
  const responseHeaders = new Headers(upstream.headers)
  responseHeaders.delete("set-cookie")
  return new Response(upstream.body, {
    status: upstream.status,
    headers: responseHeaders,
  })
}

export const GET = forward
export const POST = forward
export const PATCH = forward
export const DELETE = forward
