"use client"

import * as React from "react"
import Link from "next/link"
import { useRouter } from "next/navigation"
import { WarningCircleIcon } from "@phosphor-icons/react"

import { Alert, AlertDescription } from "@/components/ui/alert"
import { Brand } from "@/components/brand"
import { Button } from "@/components/ui/button"
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { Spinner } from "@/components/ui/spinner"
import type { Locale } from "@/lib/locales"

const copy = {
  en: {
    login: "Welcome back",
    signup: "Create your Tuenel account",
    loginDescription: "Sign in to manage your gateway tenants.",
    signupDescription: "Create an account and your first tenant.",
    verificationSent:
      "Check your email for a verification link before signing in.",
    email: "Email",
    password: "Password",
    tenant: "Organization name",
    submitLogin: "Sign in",
    submitSignup: "Create account",
    switchLogin: "Already have an account? Sign in",
    switchSignup: "Need an account? Create one",
  },
  id: {
    login: "Selamat datang kembali",
    signup: "Buat akun Tuenel",
    loginDescription: "Masuk untuk mengelola tenant gateway Anda.",
    signupDescription: "Buat akun dan tenant pertama Anda.",
    verificationSent: "Cek email Anda untuk link verifikasi sebelum masuk.",
    email: "Email",
    password: "Kata sandi",
    tenant: "Nama organisasi",
    submitLogin: "Masuk",
    submitSignup: "Buat akun",
    switchLogin: "Sudah punya akun? Masuk",
    switchSignup: "Belum punya akun? Buat akun",
  },
}

export function AuthForm({
  mode,
  locale,
  allowSignup = false,
  allowSso = false,
}: {
  mode: "login" | "signup"
  locale: Locale
  allowSignup?: boolean
  allowSso?: boolean
}) {
  const text = copy[locale]
  const router = useRouter()
  const [pending, setPending] = React.useState(false)
  const [error, setError] = React.useState<string>()
  const [verificationSent, setVerificationSent] = React.useState(false)
  const [tenantSlug, setTenantSlug] = React.useState("")

  async function submit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault()
    setPending(true)
    setError(undefined)
    const form = new FormData(event.currentTarget)
    const body = Object.fromEntries(form)
    const response = await fetch(`/api/auth/${mode}`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(body),
    })
    const result = await response.json().catch(() => ({}))
    if (!response.ok) {
      setError(result.error ?? "Authentication failed")
      setPending(false)
      return
    }
    if (mode === "signup") {
      setVerificationSent(true)
      setPending(false)
      return
    }
    const tenant = result.memberships?.[0]?.tenant_id
    if (!tenant) {
      setError("No tenant membership is available")
      setPending(false)
      return
    }
    router.replace(`/${locale}`)
    router.refresh()
  }

  async function startSso() {
    setPending(true)
    setError(undefined)
    try {
      const response = await fetch("/api/auth/sso-start", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ tenant_slug: tenantSlug, locale }),
      })
      const result = await response.json().catch(() => ({}))
      if (!response.ok) throw new Error(result.error ?? "SSO is unavailable")
      window.location.assign(result.authorization_url)
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "SSO is unavailable")
      setPending(false)
    }
  }

  return (
    <div className="w-full max-w-md">
      <Brand className="mb-5 lg:hidden" size={40} />
      <Card>
        <CardHeader>
          <CardTitle>{mode === "login" ? text.login : text.signup}</CardTitle>
          <CardDescription>
            {mode === "login" ? text.loginDescription : text.signupDescription}
          </CardDescription>
        </CardHeader>
        <CardContent>
          <form onSubmit={submit}>
            <FieldGroup>
              {error && (
                <Alert variant="destructive">
                  <WarningCircleIcon />
                  <AlertDescription>{error}</AlertDescription>
                </Alert>
              )}
              <Field>
                <FieldLabel htmlFor="email">{text.email}</FieldLabel>
                <Input
                  id="email"
                  name="email"
                  type="email"
                  autoComplete="email"
                  required
                />
              </Field>
              {mode === "signup" && (
                <Field>
                  <FieldLabel htmlFor="tenant_name">{text.tenant}</FieldLabel>
                  <Input
                    id="tenant_name"
                    name="tenant_name"
                    minLength={1}
                    maxLength={100}
                    autoComplete="organization"
                    required
                  />
                </Field>
              )}
              <Field>
                <FieldLabel htmlFor="password">{text.password}</FieldLabel>
                <Input
                  id="password"
                  name="password"
                  type="password"
                  minLength={12}
                  maxLength={128}
                  autoComplete={
                    mode === "login" ? "current-password" : "new-password"
                  }
                  required
                />
              </Field>
              {verificationSent && (
                <Alert>
                  <AlertDescription>{text.verificationSent}</AlertDescription>
                </Alert>
              )}
              <Button type="submit" disabled={pending || verificationSent}>
                {pending && <Spinner data-icon="inline-start" />}
                {mode === "login" ? text.submitLogin : text.submitSignup}
              </Button>
            </FieldGroup>
          </form>
          {mode === "login" && allowSso && (
            <div className="mt-5 space-y-3 border-t pt-5">
              <Field>
                <FieldLabel htmlFor="sso-tenant">Organization slug</FieldLabel>
                <Input
                  id="sso-tenant"
                  value={tenantSlug}
                  onChange={(event) => setTenantSlug(event.target.value)}
                  pattern="[a-z0-9]+(?:-[a-z0-9]+)*"
                  placeholder="acme"
                />
              </Field>
              <Button
                className="w-full"
                type="button"
                variant="outline"
                disabled={pending || !tenantSlug}
                onClick={() => void startSso()}
              >
                Continue with SSO
              </Button>
            </div>
          )}
        </CardContent>
        {(mode === "signup" || allowSignup) && (
          <CardFooter>
            <Button
              variant="link"
              render={
                <Link
                  href={`/${locale}/${mode === "login" ? "register" : "login"}`}
                />
              }
            >
              {mode === "login" ? text.switchSignup : text.switchLogin}
            </Button>
          </CardFooter>
        )}
      </Card>
    </div>
  )
}
