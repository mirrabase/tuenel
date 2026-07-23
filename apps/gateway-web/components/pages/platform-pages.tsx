"use client"

import * as React from "react"
import { ArrowClockwiseIcon, PlusIcon, TrashIcon } from "@phosphor-icons/react"
import { toast } from "sonner"

import { useGateway } from "@/components/gateway-provider"
import {
  DataState,
  PageHeader,
  StatusBadge,
  useGatewayData,
} from "@/components/pages/shared"
import { Button } from "@/components/ui/button"
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import {
  Field,
  FieldDescription,
  FieldGroup,
  FieldLabel,
} from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import { Textarea } from "@/components/ui/textarea"
import { type Page, gatewayFetch } from "@/lib/gateway-api"

export type PlatformKind =
  | "tenants"
  | "providers"
  | "routing"
  | "pricing"
  | "policies"
  | "ledger"
  | "system"
  | "integrations"

type RecordValue = Record<string, unknown> & {
  id?: string
  event_id?: string
  version?: number
  status?: string
  enabled?: boolean
}

const config: Record<
  Exclude<PlatformKind, "system" | "integrations">,
  { title: string; description: string; path: string; mutable: boolean }
> = {
  tenants: {
    title: "Tenants",
    description: "Durable gateway tenants visible to this operator.",
    path: "/admin/tenants",
    mutable: false,
  },
  providers: {
    title: "Providers",
    description: "Provider adapters and write-only credential configuration.",
    path: "/admin/providers",
    mutable: true,
  },
  routing: {
    title: "Model routing",
    description: "Ordered provider targets used by live inference.",
    path: "/admin/model-routes",
    mutable: true,
  },
  pricing: {
    title: "Model pricing",
    description: "Versioned model prices used for usage cost estimation.",
    path: "/admin/model-prices",
    mutable: true,
  },
  policies: {
    title: "General policies",
    description: "Tenant-scoped inference and quota policy resources.",
    path: "/admin/policies",
    mutable: true,
  },
  ledger: {
    title: "Usage ledger",
    description: "Immutable usage events from PostgreSQL.",
    path: "/admin/usage/events",
    mutable: false,
  },
}

export function PlatformPage({ kind }: { kind: PlatformKind }) {
  if (kind === "system") return <SystemPage />
  if (kind === "integrations") return <IntegrationsPage />
  return <ResourcePage {...config[kind]} />
}

function ResourcePage({
  title,
  description,
  path,
  mutable,
}: {
  title: string
  description: string
  path: string
  mutable: boolean
}) {
  const { tenantId, tenantRole, gatewayAdmin } = useGateway()
  const state = useGatewayData<Page<RecordValue>>(path)
  const [creating, setCreating] = React.useState(false)
  const [name, setName] = React.useState("")
  const [details, setDetails] = React.useState("{}")
  const [credential, setCredential] = React.useState("")
  const canWrite = gatewayAdmin || ["owner", "admin"].includes(tenantRole)
  const provider = path === "/admin/providers"

  async function create(event: React.FormEvent) {
    event.preventDefault()
    try {
      const extra = JSON.parse(details) as RecordValue
      await gatewayFetch(path, tenantId, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          ...extra,
          name,
          tenant_id: gatewayAdmin ? undefined : tenantId,
          ...(provider && credential ? { credential } : {}),
        }),
      })
      setName("")
      setDetails("{}")
      setCredential("")
      setCreating(false)
      state.reload()
      toast.success(`${title} resource created`)
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Create failed")
    }
  }

  return (
    <>
      <PageHeader
        title={title}
        description={description}
        action={
          mutable &&
          canWrite && (
            <Button onClick={() => setCreating((value) => !value)}>
              <PlusIcon data-icon="inline-start" />
              Add
            </Button>
          )
        }
      />
      {creating && (
        <Card className="mb-4">
          <CardHeader>
            <CardTitle>New resource</CardTitle>
            <CardDescription>
              Values are validated by the control plane before persistence.
            </CardDescription>
          </CardHeader>
          <CardContent>
            <form onSubmit={create}>
              <FieldGroup>
                <Field>
                  <FieldLabel htmlFor="resource-name">Name</FieldLabel>
                  <Input
                    id="resource-name"
                    value={name}
                    onChange={(event) => setName(event.target.value)}
                    required
                  />
                </Field>
                {provider && (
                  <Field>
                    <FieldLabel htmlFor="provider-credential">
                      Credential
                    </FieldLabel>
                    <Input
                      id="provider-credential"
                      type="password"
                      autoComplete="off"
                      value={credential}
                      onChange={(event) => setCredential(event.target.value)}
                    />
                    <FieldDescription>
                      Write-only; it is cleared when this form closes.
                    </FieldDescription>
                  </Field>
                )}
                <Field>
                  <FieldLabel htmlFor="resource-details">
                    JSON fields
                  </FieldLabel>
                  <Textarea
                    id="resource-details"
                    value={details}
                    onChange={(event) => setDetails(event.target.value)}
                    rows={6}
                  />
                </Field>
                <div className="flex gap-2">
                  <Button type="submit">Save</Button>
                  <Button
                    type="button"
                    variant="outline"
                    onClick={() => {
                      setCreating(false)
                      setCredential("")
                    }}
                  >
                    Cancel
                  </Button>
                </div>
              </FieldGroup>
            </form>
          </CardContent>
        </Card>
      )}
      <DataState
        loading={state.loading}
        error={state.error}
        empty={state.data?.data.length === 0}
        onRetry={state.reload}
      >
        <Card>
          <CardContent>
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Resource</TableHead>
                  <TableHead>Status</TableHead>
                  {mutable && <TableHead />}
                </TableRow>
              </TableHeader>
              <TableBody>
                {state.data?.data.map((record, index) => {
                  const id = String(record.id ?? record.event_id ?? index)
                  return (
                    <TableRow key={id}>
                      <TableCell>
                        <pre className="max-w-4xl overflow-auto text-xs whitespace-pre-wrap">
                          {JSON.stringify(record, null, 2)}
                        </pre>
                      </TableCell>
                      <TableCell>
                        <StatusBadge
                          status={
                            record.status ??
                            (record.enabled === false ? "Retired" : "Active")
                          }
                        />
                      </TableCell>
                      {mutable && (
                        <TableCell className="text-right">
                          <Button
                            size="sm"
                            variant="outline"
                            disabled={!canWrite}
                            onClick={async () => {
                              if (!window.confirm(`Retire ${id}?`)) return
                              try {
                                await gatewayFetch(`${path}/${id}`, tenantId, {
                                  method: "DELETE",
                                  headers: {
                                    "if-match": `"${record.version ?? 1}"`,
                                  },
                                })
                                state.reload()
                              } catch (error) {
                                toast.error(
                                  error instanceof Error
                                    ? error.message
                                    : "Retirement failed"
                                )
                              }
                            }}
                          >
                            <TrashIcon data-icon="inline-start" />
                            Retire
                          </Button>
                        </TableCell>
                      )}
                    </TableRow>
                  )
                })}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      </DataState>
    </>
  )
}

function SystemPage() {
  const state = useGatewayData<RecordValue>("/admin/system")
  return (
    <>
      <PageHeader
        title="System"
        description="Readiness, runtime reconciliation, provider health, and partial-state flags."
      />
      <DataState
        loading={state.loading}
        error={state.error}
        onRetry={state.reload}
      >
        <Card>
          <CardHeader>
            <CardTitle>Operational snapshot</CardTitle>
            <CardDescription>Sanitized server-side state.</CardDescription>
          </CardHeader>
          <CardContent>
            <pre className="overflow-auto text-xs whitespace-pre-wrap">
              {JSON.stringify(state.data, null, 2)}
            </pre>
          </CardContent>
        </Card>
      </DataState>
    </>
  )
}

function IntegrationsPage() {
  const { tenantId } = useGateway()
  const webhooks = useGatewayData<Page<RecordValue>>("/admin/billing/webhooks")
  const outbox = useGatewayData<Page<RecordValue>>("/admin/billing/outbox")
  return (
    <>
      <PageHeader
        title="Billing integrations"
        description="Sanitized webhook configuration and non-blocking delivery outbox."
      />
      <RecordCards title="Webhooks" state={webhooks} />
      <div className="mt-4">
        <RecordCards
          title="Outbox"
          state={outbox}
          action={async (record) => {
            await gatewayFetch(
              `/admin/billing/outbox/${record.event_id}/retry`,
              tenantId,
              { method: "POST" }
            )
            outbox.reload()
          }}
        />
      </div>
    </>
  )
}

function RecordCards({
  title,
  state,
  action,
}: {
  title: string
  state: ReturnType<typeof useGatewayData<Page<RecordValue>>>
  action?: (record: RecordValue) => Promise<void>
}) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>{title}</CardTitle>
      </CardHeader>
      <CardContent>
        <DataState
          loading={state.loading}
          error={state.error}
          empty={state.data?.data.length === 0}
          onRetry={state.reload}
        >
          <div className="flex flex-col gap-3">
            {state.data?.data.map((record, index) => (
              <div
                key={String(record.id ?? record.event_id ?? index)}
                className="flex items-start justify-between gap-3 rounded-md border p-3"
              >
                <pre className="overflow-auto text-xs whitespace-pre-wrap">
                  {JSON.stringify(record, null, 2)}
                </pre>
                {action && !record.delivered_at && (
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => action(record)}
                  >
                    <ArrowClockwiseIcon data-icon="inline-start" />
                    Retry
                  </Button>
                )}
              </div>
            ))}
          </div>
        </DataState>
      </CardContent>
    </Card>
  )
}
