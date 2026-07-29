import { NextResponse } from "next/server"

import {
  SESSION_COOKIE,
  SESSION_SECONDS,
  gatewayApiUrl,
  seal,
  sessionCredential,
} from "@/lib/server-auth"

export async function POST(
  request: Request,
  { params }: { params: Promise<{ action: string }> }
) {
  const { action } = await params
  if (action === "logout") {
    const credential = await sessionCredential()
    if (credential)
      await fetch(gatewayApiUrl("/auth/session"), {
        method: "DELETE",
        headers: { authorization: `Bearer ${credential}` },
      })
    const response = NextResponse.json({ ok: true })
    response.cookies.delete(SESSION_COOKIE)
    return response
  }
  if (action === "accept-invite") {
    const credential = await sessionCredential()
    if (!credential)
      return NextResponse.json(
        { error: "Authentication required" },
        { status: 401 }
      )
    const upstream = await fetch(gatewayApiUrl("/auth/invitations/accept"), {
      method: "POST",
      headers: {
        authorization: `Bearer ${credential}`,
        "content-type": "application/json",
      },
      body: await request.text(),
    })
    return NextResponse.json(await upstream.json().catch(() => ({})), {
      status: upstream.status,
    })
  }
  if (!["login", "signup", "verify", "verification-resend"].includes(action))
    return NextResponse.json({ error: "Not found" }, { status: 404 })

  const upstreamPath = action === "verification-resend"
    ? "/auth/verification/resend"
    : `/auth/${action}`
  const upstream = await fetch(gatewayApiUrl(upstreamPath), {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: await request.text(),
  })
  const data = await upstream.json().catch(() => ({}))
  if (!upstream.ok)
    return NextResponse.json(
      { error: data?.error?.message ?? "Authentication failed" },
      { status: upstream.status }
    )

  if (action !== "login") return NextResponse.json(data, { status: upstream.status })

  const credential = data.credential as string
  delete data.credential
  const response = NextResponse.json(data, { status: upstream.status })
  response.cookies.set(SESSION_COOKIE, await seal(credential), {
    httpOnly: true,
    secure: process.env.NODE_ENV === "production",
    sameSite: "lax",
    path: "/",
    maxAge: SESSION_SECONDS,
  })
  return response
}
