export function hasValidOrigin(request: Request) {
  const origin = request.headers.get("origin")
  const host = (
    request.headers.get("x-forwarded-host") ?? request.headers.get("host")
  )
    ?.split(",", 1)[0]
    .trim()
  const protocol = (
    request.headers.get("x-forwarded-proto") ?? new URL(request.url).protocol
  )
    .split(",", 1)[0]
    .trim()
    .replace(/:$/, "")

  if (!origin || !host || !protocol) return false
  try {
    return new URL(origin).origin === new URL(`${protocol}://${host}`).origin
  } catch {
    return false
  }
}
