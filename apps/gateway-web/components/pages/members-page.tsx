"use client"

import * as React from "react"
import { usePathname } from "next/navigation"
import { ClipboardIcon, PlusIcon, TrashIcon } from "@phosphor-icons/react"
import { toast } from "sonner"

import { useGateway } from "@/components/gateway-provider"
import {
  DataState,
  PageHeader,
  StatusBadge,
  useGatewayData,
} from "@/components/pages/shared"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import { gatewayFetch, type Page } from "@/lib/gateway-api"

type Member = {
  user_id: string
  email: string
  role: string
  created_at: string
  status?: string
}

type Invitation = {
  id: string
  email: string
  role: string
  expires_at: string
  created_at: string
}

export function MembersPage() {
  const session = useGateway()
  const locale = usePathname().split("/")[1]
  const members = useGatewayData<Page<Member>>(
    `/auth/tenants/${session.tenantId}/members`
  )
  const invitations = useGatewayData<Page<Invitation>>(
    `/auth/tenants/${session.tenantId}/invitations`
  )
  const [open, setOpen] = React.useState(false)
  const [role, setRole] = React.useState("engineer")
  const [link, setLink] = React.useState<string>()
  const [pending, setPending] = React.useState(false)
  const canManage = ["owner", "admin"].includes(session.tenantRole)

  async function invite(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault()
    setPending(true)
    try {
      const email = String(new FormData(event.currentTarget).get("email"))
      const result = await gatewayFetch<{ token: string }>(
        `/auth/tenants/${session.tenantId}/invitations`,
        session.tenantId,
        {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({ email, role }),
        }
      )
      setLink(
        `${window.location.origin}/${locale}/invite?token=${encodeURIComponent(result.token)}`
      )
      invitations.reload()
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Invitation failed")
    } finally {
      setPending(false)
    }
  }

  async function updateMember(member: Member, nextRole: string) {
    try {
      await gatewayFetch(
        `/auth/tenants/${session.tenantId}/members/${member.user_id}`,
        session.tenantId,
        {
          method: "PATCH",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({ role: nextRole }),
        }
      )
      members.reload()
      toast.success("Member role updated")
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Update failed")
    }
  }

  async function remove(path: string, reload: () => void) {
    try {
      await gatewayFetch(path, session.tenantId, { method: "DELETE" })
      reload()
      toast.success("Access removed")
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Removal failed")
    }
  }

  return (
    <>
      <PageHeader
        title="Team"
        action={
          <Button
            disabled={!canManage}
            onClick={() => {
              setLink(undefined)
              setOpen(true)
            }}
          >
            <PlusIcon data-icon="inline-start" />
            Invite member
          </Button>
        }
      />
      {!canManage && (
        <Alert className="mb-4">
          <AlertTitle>Read-only access</AlertTitle>
          <AlertDescription>
            Only organization owners and administrators can manage members.
          </AlertDescription>
        </Alert>
      )}

      <Card>
        <CardHeader>
          <CardTitle>Members</CardTitle>
          <CardDescription>
            People with durable access to this organization.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <DataState
            loading={members.loading}
            error={members.error}
            empty={members.data?.data.length === 0}
            onRetry={members.reload}
            emptyTitle="No members"
            emptyDescription="This organization has no visible memberships."
          >
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Name / email</TableHead>
                  <TableHead>Role</TableHead>
                  <TableHead>Status</TableHead>
                  <TableHead>Joined</TableHead>
                  <TableHead />
                </TableRow>
              </TableHeader>
              <TableBody>
                {members.data?.data.map((member) => (
                  <TableRow key={member.user_id}>
                    <TableCell className="font-medium">
                      {member.email}
                    </TableCell>
                    <TableCell>
                      <Select
                        value={member.role}
                        disabled={
                          !canManage || member.user_id === session.userId
                        }
                        onValueChange={(value) =>
                          value && updateMember(member, value)
                        }
                      >
                        <SelectTrigger className="w-32">
                          <SelectValue />
                        </SelectTrigger>
                        <SelectContent>
                          {["admin", "engineer", "viewer"].map((value) => (
                            <SelectItem key={value} value={value}>
                              {value}
                            </SelectItem>
                          ))}
                        </SelectContent>
                      </Select>
                    </TableCell>
                    <TableCell>
                      <StatusBadge status={member.status ?? "Active"} />
                    </TableCell>
                    <TableCell>
                      {new Date(member.created_at).toLocaleDateString()}
                    </TableCell>
                    <TableCell className="text-right">
                      <Button
                        variant="ghost"
                        size="icon-sm"
                        aria-label={`Remove ${member.email}`}
                        disabled={
                          !canManage ||
                          member.user_id === session.userId ||
                          member.role === "owner"
                        }
                        onClick={() =>
                          remove(
                            `/auth/tenants/${session.tenantId}/members/${member.user_id}`,
                            members.reload
                          )
                        }
                      >
                        <TrashIcon />
                      </Button>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </DataState>
        </CardContent>
      </Card>

      <Card className="mt-4">
        <CardHeader>
          <CardTitle>Pending invitations</CardTitle>
          <CardDescription>
            Invitations that have not been accepted or expired.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <DataState
            loading={invitations.loading}
            error={invitations.error}
            empty={invitations.data?.data.length === 0}
            onRetry={invitations.reload}
            emptyTitle="No pending invitations"
            emptyDescription="New invitations will remain visible here until accepted, revoked, or expired."
          >
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Email</TableHead>
                  <TableHead>Role</TableHead>
                  <TableHead>Expires</TableHead>
                  <TableHead />
                </TableRow>
              </TableHeader>
              <TableBody>
                {invitations.data?.data.map((invitation) => (
                  <TableRow key={invitation.id}>
                    <TableCell>{invitation.email}</TableCell>
                    <TableCell>{invitation.role}</TableCell>
                    <TableCell>
                      {new Date(invitation.expires_at).toLocaleDateString()}
                    </TableCell>
                    <TableCell className="text-right">
                      <Button
                        variant="ghost"
                        size="icon-sm"
                        aria-label={`Revoke invitation for ${invitation.email}`}
                        disabled={!canManage}
                        onClick={() =>
                          remove(
                            `/auth/tenants/${session.tenantId}/invitations/${invitation.id}`,
                            invitations.reload
                          )
                        }
                      >
                        <TrashIcon />
                      </Button>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </DataState>
        </CardContent>
      </Card>

      <Dialog open={open} onOpenChange={(value) => !pending && setOpen(value)}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Invite member</DialogTitle>
            <DialogDescription>
              Invitation links expire after seven days and are shown once.
            </DialogDescription>
          </DialogHeader>
          {link ? (
            <Alert>
              <AlertTitle>Invitation link created</AlertTitle>
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
                  <ClipboardIcon data-icon="inline-start" /> Copy link
                </Button>
              </AlertDescription>
            </Alert>
          ) : (
            <form onSubmit={invite}>
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
                      {["admin", "engineer", "viewer"].map((value) => (
                        <SelectItem key={value} value={value}>
                          {value}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </Field>
                <DialogFooter>
                  <Button type="submit" disabled={pending}>
                    {pending ? "Inviting…" : "Create invitation"}
                  </Button>
                </DialogFooter>
              </FieldGroup>
            </form>
          )}
        </DialogContent>
      </Dialog>
    </>
  )
}
