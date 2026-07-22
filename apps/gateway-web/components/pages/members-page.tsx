"use client"

import * as React from "react"
import { usePathname } from "next/navigation"
import { ClipboardIcon } from "@phosphor-icons/react"
import { toast } from "sonner"

import { useMockGateway } from "@/components/mock-provider"
import { PageHeader } from "@/components/pages/shared"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
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
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { gatewayFetch } from "@/lib/gateway-api"

export function MembersPage() {
  const { state } = useMockGateway()
  const locale = usePathname().split("/")[1]
  const [role, setRole] = React.useState("engineer")
  const [link, setLink] = React.useState<string>()
  const canInvite = ["owner", "admin"].includes(
    state.principal?.tenantRole ?? ""
  )
  return (
    <>
      <PageHeader
        title="Members"
        description="Invite an account to this tenant with a fixed RBAC role."
      />
      <Card>
        <CardHeader>
          <CardTitle>Invite member</CardTitle>
          <CardDescription>
            The invitation expires after seven days and its token is shown once.
          </CardDescription>
        </CardHeader>
        <CardContent>
          {!canInvite ? (
            <Alert>
              <AlertTitle>Owner or admin required</AlertTitle>
              <AlertDescription>
                Your role cannot manage tenant membership.
              </AlertDescription>
            </Alert>
          ) : link ? (
            <Alert>
              <AlertTitle>Invitation link</AlertTitle>
              <AlertDescription className="flex flex-col items-start gap-3 break-all">
                {link}
                <Button
                  variant="outline"
                  onClick={() =>
                    navigator.clipboard
                      .writeText(link)
                      .then(() => toast.success("Invitation copied"))
                  }
                >
                  <ClipboardIcon data-icon="inline-start" />
                  Copy link
                </Button>
              </AlertDescription>
            </Alert>
          ) : (
            <form
              onSubmit={async (event) => {
                event.preventDefault()
                const email = String(
                  new FormData(event.currentTarget).get("email")
                )
                try {
                  const result = await gatewayFetch<{ token: string }>(
                    `/auth/tenants/${state.principal!.tenantId}/invitations`,
                    state.principal!.tenantId,
                    {
                      method: "POST",
                      headers: { "content-type": "application/json" },
                      body: JSON.stringify({ email, role }),
                    }
                  )
                  setLink(
                    `${window.location.origin}/${locale}/invite?token=${encodeURIComponent(result.token)}`
                  )
                } catch (error) {
                  toast.error(
                    error instanceof Error ? error.message : "Invitation failed"
                  )
                }
              }}
            >
              <FieldGroup>
                <Field>
                  <FieldLabel htmlFor="invite-email">Email</FieldLabel>
                  <Input id="invite-email" name="email" type="email" required />
                </Field>
                <Field>
                  <FieldLabel>Role</FieldLabel>
                  <Select
                    value={role}
                    onValueChange={(value) => value && setRole(value)}
                  >
                    <SelectTrigger className="w-full">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectGroup>
                        {(["admin", "engineer", "viewer"] as const).map(
                          (value) => (
                            <SelectItem key={value} value={value}>
                              {value}
                            </SelectItem>
                          )
                        )}
                      </SelectGroup>
                    </SelectContent>
                  </Select>
                </Field>
                <Button type="submit">Create invitation</Button>
              </FieldGroup>
            </form>
          )}
        </CardContent>
      </Card>
    </>
  )
}
