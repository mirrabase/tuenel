"use client"

import * as React from "react"
import { useRouter } from "next/navigation"

import { Alert, AlertDescription } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { Spinner } from "@/components/ui/spinner"
import type { Locale } from "@/lib/locales"

export function SetupForm({ locale }: { locale: Locale }) {
  const router = useRouter()
  const tokenInput = React.useRef<HTMLInputElement>(null)
  const [pending, setPending] = React.useState(false)
  const [error, setError] = React.useState<string>()

  React.useEffect(() => {
    const fragment = new URLSearchParams(window.location.hash.slice(1))
    const value = fragment.get("token")
    if (value && tokenInput.current) tokenInput.current.value = value
    window.history.replaceState(null, "", window.location.pathname)
  }, [])

  async function submit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault()
    setPending(true)
    setError(undefined)
    const body = Object.fromEntries(new FormData(event.currentTarget))
    const response = await fetch("/api/auth/bootstrap", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(body),
    })
    const result = await response.json().catch(() => ({}))
    if (!response.ok) {
      setError(result.error ?? "Installation setup failed")
      setPending(false)
      return
    }
    router.replace(`/${locale}`)
    router.refresh()
  }

  return (
    <Card className="w-full max-w-md">
      <CardHeader>
        <CardTitle>Initialize Tuenel</CardTitle>
        <CardDescription>
          Create the instance administrator and first organization.
        </CardDescription>
      </CardHeader>
      <CardContent>
        <form onSubmit={submit}>
          <FieldGroup>
            {error && (
              <Alert variant="destructive">
                <AlertDescription>{error}</AlertDescription>
              </Alert>
            )}
            <Field>
              <FieldLabel htmlFor="token">One-time setup token</FieldLabel>
              <Input
                id="token"
                name="token"
                type="password"
                ref={tokenInput}
                required
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="email">Administrator email</FieldLabel>
              <Input id="email" name="email" type="email" required />
            </Field>
            <Field>
              <FieldLabel htmlFor="password">Password</FieldLabel>
              <Input
                id="password"
                name="password"
                type="password"
                minLength={12}
                maxLength={128}
                required
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="tenant_name">Organization name</FieldLabel>
              <Input
                id="tenant_name"
                name="tenant_name"
                minLength={1}
                maxLength={100}
                required
              />
            </Field>
            <Button type="submit" disabled={pending}>
              {pending && <Spinner data-icon="inline-start" />}
              Initialize instance
            </Button>
          </FieldGroup>
        </form>
      </CardContent>
    </Card>
  )
}
