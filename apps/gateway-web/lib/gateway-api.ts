export class GatewayApiError extends Error {
  readonly status: number
  readonly code?: string
  readonly requestId?: string

  constructor(
    message: string,
    status: number,
    code?: string,
    requestId?: string
  ) {
    super(message)
    this.status = status
    this.code = code
    this.requestId = requestId
  }
}

export type Page<T> = { data: T[]; next_cursor: string | null }

export async function gatewayResponse(
  path: string,
  tenantId: string,
  init?: RequestInit,
  projectId?: string
) {
  const url = new URL(`/api/gateway${path}`, window.location.origin)
  url.searchParams.set("tenant", tenantId)
  const headers = new Headers(init?.headers)
  if (projectId) headers.set("x-tuenel-project-id", projectId)
  const response = await fetch(url, { ...init, headers })
  if (!response.ok) {
    const body = await response.json().catch(() => ({}))
    throw new GatewayApiError(
      body?.error?.message ?? body?.error ?? "Gateway request failed",
      response.status,
      body?.error?.code,
      response.headers.get("x-request-id") ?? undefined
    )
  }
  return response
}

export async function gatewayFetch<T>(
  path: string,
  tenantId: string,
  init?: RequestInit,
  projectId?: string
): Promise<T> {
  const response = await gatewayResponse(path, tenantId, init, projectId)
  if (response.status === 204) return undefined as T
  return response.json()
}

export async function readSse(
  response: Response,
  onEvent: (event: unknown) => void,
  signal?: AbortSignal
) {
  const reader = response.body?.getReader()
  if (!reader) throw new Error("Streaming response has no body")
  const decoder = new TextDecoder()
  let buffer = ""
  const abort = () => void reader.cancel()
  signal?.addEventListener("abort", abort, { once: true })
  try {
    while (true) {
      const { done, value } = await reader.read()
      if (done) break
      buffer += decoder.decode(value, { stream: true }).replaceAll("\r\n", "\n")
      let boundary = buffer.indexOf("\n\n")
      while (boundary >= 0) {
        const block = buffer.slice(0, boundary)
        buffer = buffer.slice(boundary + 2)
        for (const line of block.split("\n")) {
          if (!line.startsWith("data:")) continue
          const data = line.slice(5).trimStart()
          if (data !== "[DONE]") onEvent(JSON.parse(data))
        }
        boundary = buffer.indexOf("\n\n")
      }
    }
  } finally {
    signal?.removeEventListener("abort", abort)
    reader.releaseLock()
  }
}
