"use client"

import * as React from "react"
import { useParams, useRouter, useSearchParams } from "next/navigation"

import { Alert, AlertDescription } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"

export default function InvitePage() {
  const token = useSearchParams().get("token")
  const { locale } = useParams<{ locale: string }>()
  const router = useRouter()
  const [error, setError] = React.useState<string>()
  const [pending, setPending] = React.useState(false)
  return (
    <Card className="w-full max-w-md">
      <CardHeader>
        <CardTitle>Join tenant</CardTitle>
        <CardDescription>
          Accept this invitation with your signed-in Tuenel account.
        </CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        {error && (
          <Alert variant="destructive">
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        )}
        <Button
          disabled={!token || pending}
          onClick={async () => {
            setPending(true)
            const response = await fetch("/api/auth/accept-invite", {
              method: "POST",
              headers: { "content-type": "application/json" },
              body: JSON.stringify({ token }),
            })
            const result = await response.json().catch(() => ({}))
            if (!response.ok) {
              setError(
                result?.error?.message ??
                  result.error ??
                  "Invitation could not be accepted"
              )
              setPending(false)
              return
            }
            router.replace(`/${locale}/${result.membership.tenant_id}`)
            router.refresh()
          }}
        >
          {pending ? "Joining…" : "Accept invitation"}
        </Button>
      </CardContent>
    </Card>
  )
}
