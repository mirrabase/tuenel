export async function gatewayFetch<T>(
  path: string,
  tenantId: string,
  init?: RequestInit
): Promise<T> {
  const separator = path.includes("?") ? "&" : "?"
  const response = await fetch(
    `/api/gateway${path}${separator}tenant=${encodeURIComponent(tenantId)}`,
    init
  )
  const body = await response.json().catch(() => ({}))
  if (!response.ok)
    throw new Error(
      body?.error?.message ?? body?.error ?? "Gateway request failed"
    )
  return body
}
