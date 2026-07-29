"use client"

import * as React from "react"
import { useParams, useRouter } from "next/navigation"

import { Alert, AlertDescription } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import { Field, FieldLabel } from "@/components/ui/field"
import { Input } from "@/components/ui/input"

export default function InvitePage() {
  const { locale } = useParams<{ locale: string }>()
  const router = useRouter()
  const [token, setToken] = React.useState("")
  const [password, setPassword] = React.useState("")
  const [error, setError] = React.useState<string>()
  const [pending, setPending] = React.useState(false)

  React.useEffect(() => {
    const fragment = new URLSearchParams(window.location.hash.slice(1))
    const legacyQuery = new URLSearchParams(window.location.search)
    const urlToken = fragment.get("token") ?? legacyQuery.get("token") ?? ""
    queueMicrotask(() => setToken(urlToken))
    window.history.replaceState(null, "", window.location.pathname)
  }, [])

  async function join() {
    setPending(true)
    setError(undefined)
    const response = await fetch("/api/auth/accept-invite", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ token }),
    })
    const result = await response.json().catch(() => ({}))
    if (response.ok) {
      router.replace(`/${locale}/${result.membership.tenant_id}`)
      router.refresh()
      return
    }
    if (response.status === 401 && password) {
      const registration = await fetch("/api/auth/invitation-register", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ token, password }),
      })
      const registered = await registration.json().catch(() => ({}))
      if (registration.ok) {
        router.replace(`/${locale}`)
        router.refresh()
        return
      }
      setError(
        registered.error ?? "Invitation registration could not be completed"
      )
      setPending(false)
      return
    }
    setError(
      response.status === 401
        ? "Sign in first or enter a password to create your account."
        : (result?.error?.message ??
            result.error ??
            "Invitation could not be accepted")
    )
    setPending(false)
  }

  return (
    <Card className="w-full max-w-md">
      <CardHeader>
        <CardTitle>Join organization</CardTitle>
        <CardDescription>
          Signed-in users can join directly. New users can choose a password to
          create their account.
        </CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        {error && (
          <Alert variant="destructive">
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        )}
        <Field>
          <FieldLabel htmlFor="invitation-password">
            Password for a new account
          </FieldLabel>
          <Input
            id="invitation-password"
            type="password"
            minLength={12}
            maxLength={128}
            value={password}
            onChange={(event) => setPassword(event.target.value)}
          />
        </Field>
        <Button disabled={!token || pending} onClick={join}>
          {pending ? "Joining…" : "Accept invitation"}
        </Button>
      </CardContent>
    </Card>
  )
}
