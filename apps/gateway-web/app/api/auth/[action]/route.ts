import { NextResponse } from "next/server"

import {
  SESSION_COOKIE,
  SESSION_SECONDS,
  gatewayApiUrl,
  seal,
  sessionCredential,
} from "@/lib/server-auth"
import { hasValidOrigin } from "@/lib/request-origin"

const sessionCookieSecure = process.env.WEB_SESSION_COOKIE_SECURE !== "false"

export async function GET(
  request: Request,
  { params }: { params: Promise<{ action: string }> }
) {
  const { action } = await params
  if (action === "sso-callback") {
    const url = new URL(request.url)
    const tenantSlug = url.searchParams.get("tenant_slug") ?? ""
    const locale = url.searchParams.get("locale") === "id" ? "id" : "en"
    const state = url.searchParams.get("state") ?? ""
    const code = url.searchParams.get("code") ?? ""
    if (!validTenantSlug(tenantSlug) || !state || !code)
      return NextResponse.redirect(
        new URL(`/${locale}/login?sso=invalid`, request.url)
      )
    const callback = await fetch(
      gatewayApiUrl(
        `/commercial/sso/${encodeURIComponent(tenantSlug)}/callback?${new URLSearchParams({ state, code })}`
      ),
      { cache: "no-store" }
    )
    const callbackData = await callback.json().catch(() => ({}))
    if (!callback.ok)
      return NextResponse.redirect(
        new URL(`/${locale}/login?sso=unavailable`, request.url)
      )
    const exchange = await fetch(gatewayApiUrl("/commercial/sso/exchange"), {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ exchange_code: callbackData.exchange_code }),
    })
    const exchangeData = await exchange.json().catch(() => ({}))
    if (!exchange.ok || typeof exchangeData.credential !== "string")
      return NextResponse.redirect(
        new URL(`/${locale}/login?sso=unavailable`, request.url)
      )
    const response = NextResponse.redirect(new URL(`/${locale}`, request.url))
    response.cookies.set(
      SESSION_COOKIE,
      await seal(exchangeData.credential as string),
      {
        httpOnly: true,
        secure: process.env.NODE_ENV === "production",
        sameSite: "lax",
        path: "/",
        maxAge: SESSION_SECONDS,
      }
    )
    return response
  }
  if (action !== "capabilities")
    return NextResponse.json({ error: "Not found" }, { status: 404 })
  const upstream = await fetch(gatewayApiUrl("/auth/capabilities"), {
    cache: "no-store",
  })
  return NextResponse.json(await upstream.json().catch(() => ({})), {
    status: upstream.status,
  })
}

export async function POST(
  request: Request,
  { params }: { params: Promise<{ action: string }> }
) {
  const { action } = await params
  if (!hasValidOrigin(request))
    return NextResponse.json(
      { error: "Invalid request origin" },
      { status: 403 }
    )
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
  if (action === "sso-start") {
    const input = await request.json().catch(() => ({}))
    const tenantSlug =
      typeof input.tenant_slug === "string" ? input.tenant_slug : ""
    const locale = input.locale === "id" ? "id" : "en"
    if (!validTenantSlug(tenantSlug))
      return NextResponse.json({ error: "SSO is unavailable" }, { status: 404 })
    const redirect = new URL("/api/auth/sso-callback", request.url)
    redirect.searchParams.set("tenant_slug", tenantSlug)
    redirect.searchParams.set("locale", locale)
    const upstream = await fetch(
      gatewayApiUrl(
        `/commercial/sso/${encodeURIComponent(tenantSlug)}/start`
      ),
      {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ redirect_uri: redirect.toString() }),
      }
    )
    const data = await upstream.json().catch(() => ({}))
    return NextResponse.json(
      upstream.ok
        ? data
        : {
            error: "SSO is unavailable",
          },
      { status: upstream.status }
    )
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
  if (
    ![
      "login",
      "signup",
      "verify",
      "verification-resend",
      "bootstrap",
      "invitation-register",
    ].includes(action)
  )
    return NextResponse.json({ error: "Not found" }, { status: 404 })

  const upstreamPath =
    action === "verification-resend"
      ? "/auth/verification/resend"
      : action === "invitation-register"
        ? "/auth/invitations/register"
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

  if (!["login", "bootstrap", "invitation-register"].includes(action))
    return NextResponse.json(data, { status: upstream.status })

  const credential = data.credential as string
  delete data.credential
  const response = NextResponse.json(data, { status: upstream.status })
  response.cookies.set(SESSION_COOKIE, await seal(credential), {
    httpOnly: true,
    secure: sessionCookieSecure,
    sameSite: "lax",
    path: "/",
    maxAge: SESSION_SECONDS,
  })
  return response
}

function validTenantSlug(value: string) {
  return /^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(value) && value.length <= 63
}
