import { NextResponse, type NextRequest } from "next/server"

export function proxy(request: NextRequest) {
  const first = request.nextUrl.pathname.split("/")[1]
  if (
    ["en", "id", "api", "_next"].includes(first) ||
    request.nextUrl.pathname.includes(".")
  )
    return NextResponse.next()
  return NextResponse.redirect(new URL("/en/login", request.url))
}
