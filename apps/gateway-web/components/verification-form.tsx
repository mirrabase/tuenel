"use client"

import * as React from "react"
import Link from "next/link"

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

export function VerificationForm({ locale }: { locale: Locale }) {
  const [email, setEmail] = React.useState("")
  const [message, setMessage] = React.useState("")
  const [error, setError] = React.useState<string>()
  const [pending, setPending] = React.useState(false)

  React.useEffect(() => {
    const fragment = new URLSearchParams(window.location.hash.slice(1))
    const legacyQuery = new URLSearchParams(window.location.search)
    const value = fragment.get("token") ?? legacyQuery.get("token") ?? ""
    window.history.replaceState(null, "", window.location.pathname)
    if (!value) return
    async function verify() {
      setPending(true)
      setMessage("Verifying your email...")
      const response = await fetch("/api/auth/verify", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ token: value }),
      })
      const result = await response.json().catch(() => ({}))
      setPending(false)
      if (!response.ok) setError(result.error ?? "Verification failed")
      else {
        setEmail(result.email ?? "")
        setMessage("Email verified. You can sign in now.")
      }
    }
    void verify()
  }, [])

  async function resend(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault()
    setPending(true)
    setError(undefined)
    const response = await fetch("/api/auth/verification-resend", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ email }),
    })
    setPending(false)
    if (!response.ok) setError("Could not resend verification email")
    else
      setMessage(
        "If the account is pending, a new verification email was sent."
      )
  }

  return (
    <Card className="w-full max-w-md">
      <CardHeader>
        <CardTitle>Verify your email</CardTitle>
        <CardDescription>
          {message || "Enter your email to resend the verification link."}
        </CardDescription>
      </CardHeader>
      <CardContent>
        {error && (
          <Alert variant="destructive">
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        )}
        {!message.includes("verified") && (
          <form onSubmit={resend}>
            <FieldGroup>
              <Field>
                <FieldLabel htmlFor="email">Email</FieldLabel>
                <Input
                  id="email"
                  type="email"
                  value={email}
                  onChange={(event) => setEmail(event.target.value)}
                  required
                />
              </Field>
              <Button type="submit" disabled={pending}>
                {pending && <Spinner data-icon="inline-start" />}Resend
                verification
              </Button>
            </FieldGroup>
          </form>
        )}
        {message.includes("verified") && (
          <Button render={<Link href={`/${locale}/login`} />}>
            Go to sign in
          </Button>
        )}
      </CardContent>
    </Card>
  )
}
