"use client"

import * as React from "react"
import Link from "next/link"
import { useRouter } from "next/navigation"
import { WarningCircleIcon } from "@phosphor-icons/react"

import { Alert, AlertDescription } from "@/components/ui/alert"
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
    email: "Email",
    password: "Password",
    tenant: "Tenant name",
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
    email: "Email",
    password: "Kata sandi",
    tenant: "Nama tenant",
    submitLogin: "Masuk",
    submitSignup: "Buat akun",
    switchLogin: "Sudah punya akun? Masuk",
    switchSignup: "Belum punya akun? Buat akun",
  },
}

export function AuthForm({
  mode,
  locale,
}: {
  mode: "login" | "signup"
  locale: Locale
}) {
  const text = copy[locale]
  const router = useRouter()
  const [pending, setPending] = React.useState(false)
  const [error, setError] = React.useState<string>()

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
    const tenant = result.memberships?.[0]?.tenant_id
    if (!tenant) {
      setError("No tenant membership is available")
      setPending(false)
      return
    }
    router.replace(mode === "signup" ? `/${locale}/${tenant}/projects/new` : `/${locale}`)
    router.refresh()
  }

  return (
    <Card className="w-full max-w-md">
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
            {mode === "signup" && (
              <Field>
                <FieldLabel htmlFor="tenant_name">{text.tenant}</FieldLabel>
                <Input
                  id="tenant_name"
                  name="tenant_name"
                  maxLength={100}
                  required
                />
              </Field>
            )}
            <Button type="submit" disabled={pending}>
              {pending && <Spinner data-icon="inline-start" />}
              {mode === "login" ? text.submitLogin : text.submitSignup}
            </Button>
          </FieldGroup>
        </form>
      </CardContent>
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
    </Card>
  )
}
