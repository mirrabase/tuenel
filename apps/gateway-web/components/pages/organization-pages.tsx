"use client"

import * as React from "react"
import { usePathname, useRouter } from "next/navigation"
import { ArrowSquareOutIcon, PlusIcon } from "@phosphor-icons/react"
import { toast } from "sonner"

import { useGateway } from "@/components/gateway-provider"
import { MembersPage } from "@/components/pages/members-page"
import {
  DataState,
  Metric,
  PageHeader,
  StatusBadge,
  useGatewayData,
} from "@/components/pages/shared"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Badge } from "@/components/ui/badge"
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

type JsonRecord = Record<string, unknown>

export function OrganizationTeamPage() {
  return <MembersPage />
}

type Provider = {
  id: string
  name?: string
  provider_type?: string
  enabled?: boolean
  credential_configured?: boolean
  version?: number
  available_models?: string[]
  models_synced_at?: string
}

export function OrganizationProvidersPage() {
  const { tenantId, tenantRole, gatewayAdmin } = useGateway()
  const providers = useGatewayData<Page<Provider>>(
    `/admin/providers?tenant_id=${tenantId}`
  )
  const health = useGatewayData<JsonRecord>(
    `/admin/system?tenant_id=${tenantId}`
  )
  const [open, setOpen] = React.useState(false)
  const [pending, setPending] = React.useState(false)
  const [activating, setActivating] = React.useState<string[]>([])
  const canWrite = gatewayAdmin || ["owner", "admin"].includes(tenantRole)
  const healthRows = Array.isArray(health.data?.providers)
    ? (health.data.providers as JsonRecord[])
    : []

  function modelsFor(providerId: string) {
    return (
      providers.data?.data.find((provider) => provider.id === providerId)
        ?.available_models ?? []
    )
  }

  async function syncProvider(providerId: string) {
    setActivating((current) => [...new Set([...current, providerId])])
    for (let attempt = 0; attempt < 8; attempt += 1) {
      try {
        await gatewayFetch(
          `/admin/providers/${encodeURIComponent(providerId)}/check`,
          tenantId,
          {
            method: "POST",
          }
        )
        await gatewayFetch(
          `/admin/providers/${encodeURIComponent(providerId)}/models`,
          tenantId,
          {
            method: "POST",
          }
        )
        providers.reload()
        health.reload()
        setActivating((current) => current.filter((id) => id !== providerId))
        toast.success("Provider is healthy and models are synchronized")
        return
      } catch {
        await new Promise((resolve) => window.setTimeout(resolve, 1000))
      }
    }
    providers.reload()
    health.reload()
    setActivating((current) => current.filter((id) => id !== providerId))
    toast.warning("Automatic provider verification needs attention")
  }

  async function monitorProvider(providerId: string) {
    setActivating((current) => [...new Set([...current, providerId])])
    for (let attempt = 0; attempt < 16; attempt += 1) {
      await new Promise((resolve) => window.setTimeout(resolve, 750))
      try {
        const [providerPage, system] = await Promise.all([
          gatewayFetch<Page<Provider>>(
            `/admin/providers?tenant_id=${encodeURIComponent(tenantId)}`,
            tenantId
          ),
          gatewayFetch<JsonRecord>(
            `/admin/system?tenant_id=${encodeURIComponent(tenantId)}`,
            tenantId
          ),
        ])
        const provider = providerPage.data.find(
          (item) => item.id === providerId
        )
        const rows = Array.isArray(system.providers)
          ? (system.providers as JsonRecord[])
          : []
        const providerHealth = rows.find(
          (row) => String(row.provider_id) === providerId
        )
        providers.reload()
        health.reload()
        if (
          provider?.available_models?.length &&
          providerHealth?.status === "healthy"
        ) {
          setActivating((current) => current.filter((id) => id !== providerId))
          toast.success("Provider is healthy and models are synchronized")
          return
        }
      } catch {
        // The durable backend activation keeps running if this browser poll fails.
      }
    }
    setActivating((current) => current.filter((id) => id !== providerId))
    toast.warning("Provider saved; automatic verification is still pending")
  }

  async function create(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const form = new FormData(event.currentTarget)
    const providerType = String(form.get("provider_type"))
    setPending(true)
    try {
      await gatewayFetch("/admin/providers", tenantId, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          id: form.get("id"),
          name: form.get("name"),
          provider_type: providerType,
          base_url:
            providerType === "openai"
              ? "https://api.openai.com/v1/"
              : form.get("base_url"),
          credential: form.get("credential"),
          tenant_id: tenantId,
        }),
      })
      setOpen(false)
      providers.reload()
      toast.success(
        "Provider added. Verifying health and models automatically…"
      )
      void monitorProvider(String(form.get("id")))
    } catch (error) {
      toast.error(
        error instanceof Error ? error.message : "Provider creation failed"
      )
    } finally {
      setPending(false)
    }
  }

  return (
    <>
      <PageHeader
        title="Providers"
        action={
          <Button disabled={!canWrite} onClick={() => setOpen(true)}>
            <PlusIcon data-icon="inline-start" />
            Add provider
          </Button>
        }
      />
      <Card>
        <CardHeader>
          <CardTitle>Organization providers</CardTitle>
          <CardDescription>
            Credentials remain write-only; projects reference these adapters
            through routing rules.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <DataState
            loading={providers.loading}
            error={providers.error}
            onRetry={providers.reload}
          >
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Provider</TableHead>
                  <TableHead>Status</TableHead>
                  <TableHead>Available models</TableHead>
                  <TableHead>Credential</TableHead>
                  <TableHead>Last health check</TableHead>
                  <TableHead />
                </TableRow>
              </TableHeader>
              <TableBody>
                {providers.data?.data.length === 0 && (
                  <TableRow>
                    <TableCell colSpan={6} className="py-10 text-center">
                      <p className="font-medium">No organization providers</p>
                      <p className="mt-1 text-sm text-muted-foreground">
                        Add a provider to make its models and credential
                        available to projects.
                      </p>
                    </TableCell>
                  </TableRow>
                )}
                {providers.data?.data.map((provider) => {
                  const providerHealth = healthRows.find(
                    (row) => String(row.provider_id) === provider.id
                  )
                  const models = modelsFor(provider.id)
                  const checking = activating.includes(provider.id)
                  const ready =
                    providerHealth?.status === "healthy" && models.length > 0
                  const status = checking
                    ? "Checking"
                    : ready
                      ? "Healthy"
                      : providerHealth?.status === "unhealthy"
                        ? "Unhealthy"
                        : models.length
                          ? "Health pending"
                          : "Model sync pending"
                  return (
                    <TableRow key={provider.id}>
                      <TableCell>
                        <div className="font-medium">
                          {provider.name ?? provider.id}
                        </div>
                        <div className="text-xs text-muted-foreground">
                          {provider.provider_type}
                        </div>
                      </TableCell>
                      <TableCell>
                        <StatusBadge
                          status={
                            provider.enabled === false ? "Disabled" : status
                          }
                        />
                      </TableCell>
                      <TableCell>
                        {models.length ? models.join(", ") : "Not synced"}
                      </TableCell>
                      <TableCell>
                        <StatusBadge
                          status={
                            provider.credential_configured
                              ? "Configured"
                              : "Missing"
                          }
                        />
                      </TableCell>
                      <TableCell>
                        {providerHealth?.updated_at
                          ? new Date(
                              String(providerHealth.updated_at)
                            ).toLocaleString()
                          : "Never"}
                      </TableCell>
                      <TableCell className="text-right">
                        <Button
                          size="sm"
                          variant="outline"
                          disabled={!canWrite || checking || ready}
                          onClick={() => void syncProvider(provider.id)}
                        >
                          {checking
                            ? "Checking…"
                            : ready
                              ? "Ready"
                              : "Retry setup"}
                        </Button>
                      </TableCell>
                    </TableRow>
                  )
                })}
              </TableBody>
            </Table>
          </DataState>
        </CardContent>
      </Card>

      <Dialog open={open} onOpenChange={(value) => !pending && setOpen(value)}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Add organization provider</DialogTitle>
            <DialogDescription>
              The credential is stored through the gateway secret service and is
              never returned.
            </DialogDescription>
          </DialogHeader>
          <form onSubmit={create}>
            <FieldGroup>
              <Field>
                <FieldLabel htmlFor="provider-id">Provider ID</FieldLabel>
                <Input id="provider-id" name="id" required maxLength={100} />
              </Field>
              <Field>
                <FieldLabel htmlFor="provider-name">Display name</FieldLabel>
                <Input
                  id="provider-name"
                  name="name"
                  required
                  maxLength={100}
                />
              </Field>
              <Field>
                <FieldLabel>Provider type</FieldLabel>
                <Select name="provider_type" defaultValue="openai_compatible">
                  <SelectTrigger className="w-full">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="openai">OpenAI</SelectItem>
                    <SelectItem value="openai_compatible">
                      OpenAI-compatible
                    </SelectItem>
                    <SelectItem value="anthropic">Anthropic</SelectItem>
                    <SelectItem value="gemini">Gemini</SelectItem>
                  </SelectContent>
                </Select>
              </Field>
              <Field>
                <FieldLabel htmlFor="provider-url">Base URL</FieldLabel>
                <Input
                  id="provider-url"
                  name="base_url"
                  type="url"
                  placeholder="OpenAI uses https://api.openai.com/v1/"
                  defaultValue="https://api.openai.com/v1/"
                  required
                />
              </Field>
              <Field>
                <FieldLabel htmlFor="provider-credential">
                  Credential
                </FieldLabel>
                <Input
                  id="provider-credential"
                  name="credential"
                  type="password"
                  autoComplete="new-password"
                />
              </Field>
              <DialogFooter>
                <Button type="submit" disabled={pending}>
                  {pending ? "Adding…" : "Add provider"}
                </Button>
              </DialogFooter>
            </FieldGroup>
          </form>
        </DialogContent>
      </Dialog>
    </>
  )
}

export function ProjectProvidersPage() {
  const { tenantId, projectId } = useGateway()
  const providers = useGatewayData<Page<Provider>>(
    `/admin/providers?tenant_id=${tenantId}`
  )
  const routes = useGatewayData<Page<JsonRecord>>(
    `/admin/model-routes?tenant_id=${tenantId}&project_id=${encodeURIComponent(projectId ?? "")}`
  )
  return (
    <>
      <PageHeader title="Project Providers" />
      <Card>
        <CardHeader>
          <CardTitle>Available adapters</CardTitle>
          <CardDescription>
            Project routing rules select from these shared organization
            providers.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <DataState
            loading={providers.loading}
            error={providers.error}
            onRetry={providers.reload}
          >
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Provider</TableHead>
                  <TableHead>Status</TableHead>
                  <TableHead>Models routed in this project</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {providers.data?.data.length === 0 && (
                  <TableRow>
                    <TableCell colSpan={3} className="py-10 text-center">
                      <p className="font-medium">No providers available</p>
                      <p className="mt-1 text-sm text-muted-foreground">
                        Ask an organization administrator to add a provider.
                      </p>
                    </TableCell>
                  </TableRow>
                )}
                {providers.data?.data.map((provider) => {
                  const models = (routes.data?.data ?? [])
                    .filter(
                      (route) =>
                        String(route.provider ?? route.provider_id) ===
                        provider.id
                    )
                    .map((route) =>
                      String(
                        route.requested_model ?? route.upstream_model ?? ""
                      )
                    )
                    .filter(Boolean)
                  return (
                    <TableRow key={provider.id}>
                      <TableCell className="font-medium">
                        {provider.name ?? provider.id}
                      </TableCell>
                      <TableCell>
                        <StatusBadge
                          status={
                            provider.enabled === false
                              ? "Disabled"
                              : "Available"
                          }
                        />
                      </TableCell>
                      <TableCell>
                        {models.length ? models.join(", ") : "Not routed"}
                      </TableCell>
                    </TableRow>
                  )
                })}
              </TableBody>
            </Table>
          </DataState>
        </CardContent>
      </Card>
    </>
  )
}

type UsageSummary = {
  requests?: number
  input_tokens?: number
  output_tokens?: number
  total_tokens?: number
  estimated_cost?: number
  unpriced_requests?: number
  success_rate?: number
}

type UsageBreakdown = {
  projects?: JsonRecord[]
  providers?: JsonRecord[]
  models?: JsonRecord[]
  api_keys?: JsonRecord[]
}

const ranges = { "24h": 1, "7d": 7, "30d": 30 } as const

export function OrganizationUsagePage() {
  const { tenantId } = useGateway()
  const [range, setRange] = React.useState<keyof typeof ranges>("7d")
  const from = React.useMemo(() => {
    const date = new Date()
    date.setUTCDate(date.getUTCDate() - ranges[range])
    return date.toISOString()
  }, [range])
  const [to] = React.useState(() => new Date().toISOString())
  const query = `tenant_id=${tenantId}&from=${encodeURIComponent(from)}&to=${encodeURIComponent(to)}`
  const summary = useGatewayData<UsageSummary>(`/admin/usage/summary?${query}`)
  const series = useGatewayData<{ data: JsonRecord[] }>(
    `/admin/usage/series?${query}&limit=100`
  )
  const breakdowns = useGatewayData<UsageBreakdown>(
    `/admin/usage/breakdowns?${query}`
  )
  const billing = useGatewayData<JsonRecord>(
    `/admin/billing/overview?tenant_id=${tenantId}`
  )
  const values = summary.data ?? {}
  const points = series.data?.data ?? []
  const maxRequests = Math.max(
    1,
    ...points.map((point) => Number(point.requests ?? 0))
  )
  const maxCost = Math.max(
    0.000001,
    ...points.map((point) => Number(point.cost ?? 0))
  )
  const allowance = Number(billing.data?.request_allowance ?? 0)
  const consumption = Number(values.requests ?? 0)

  return (
    <>
      <PageHeader
        title="Organization Usage"
        action={
          <div className="flex gap-1 rounded-md border p-1">
            {Object.keys(ranges).map((value) => (
              <Button
                key={value}
                size="sm"
                variant={range === value ? "secondary" : "ghost"}
                onClick={() => setRange(value as keyof typeof ranges)}
              >
                {value}
              </Button>
            ))}
          </div>
        }
      />
      <DataState
        loading={summary.loading}
        error={summary.error}
        onRetry={summary.reload}
      >
        <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-5">
          <Metric
            label="Total requests"
            value={Number(values.requests ?? 0).toLocaleString()}
            detail="All organization projects"
          />
          <Metric
            label="Input tokens"
            value={Number(values.input_tokens ?? 0).toLocaleString()}
            detail="Prompt consumption"
          />
          <Metric
            label="Output tokens"
            value={Number(values.output_tokens ?? 0).toLocaleString()}
            detail="Completion consumption"
          />
          <Metric
            label="Estimated cost"
            value={`$${Number(values.estimated_cost ?? 0).toFixed(4)}`}
            detail={
              Number(values.unpriced_requests ?? 0) > 0
                ? `${Number(values.unpriced_requests).toLocaleString()} requests unpriced`
                : "Provider cost"
            }
          />
          <Metric
            label="Success rate"
            value={`${Number(values.success_rate ?? 0).toFixed(1)}%`}
            detail="Succeeded requests"
          />
        </div>
      </DataState>

      <div className="mt-6 grid gap-4 xl:grid-cols-2">
        <UsageChart
          title="Requests over time"
          description="Aggregate request volume"
          points={points}
          field="requests"
          max={maxRequests}
          loading={series.loading}
          error={series.error}
        />
        <UsageChart
          title="Cost over time"
          description="Estimated provider cost"
          points={points}
          field="cost"
          max={maxCost}
          loading={series.loading}
          error={series.error}
        />
      </div>

      {Boolean(billing.data?.configured) && (
        <Card className="mt-4">
          <CardHeader>
            <CardTitle>Plan allowance</CardTitle>
            <CardDescription>
              Current organization consumption for the selected period.
            </CardDescription>
          </CardHeader>
          <CardContent>
            {billing.error ? (
              <Alert>
                <AlertTitle>Billing configuration unavailable</AlertTitle>
                <AlertDescription>
                  Usage remains available while plan allowances are configured.
                </AlertDescription>
              </Alert>
            ) : (
              <>
                <div className="mb-2 flex justify-between text-sm">
                  <span>{consumption.toLocaleString()} requests</span>
                  <span>
                    {allowance
                      ? `${allowance.toLocaleString()} allowed`
                      : "No allowance configured"}
                  </span>
                </div>
                <div className="h-2 overflow-hidden rounded-full bg-muted">
                  <div
                    className="h-full bg-primary"
                    style={{
                      width: `${allowance ? Math.min(100, (consumption / allowance) * 100) : 0}%`,
                    }}
                  />
                </div>
              </>
            )}
          </CardContent>
        </Card>
      )}

      <Card className="mt-4">
        <CardHeader>
          <CardTitle>Project comparison</CardTitle>
          <CardDescription>
            Measured usage by project; no values are inferred or evenly
            distributed.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <BreakdownTable
            rows={breakdowns.data?.projects ?? []}
            label="Project"
            loading={breakdowns.loading}
            error={breakdowns.error}
          />
        </CardContent>
      </Card>

      <div className="mt-4 grid gap-4 xl:grid-cols-3">
        <BreakdownCard
          title="By provider"
          rows={breakdowns.data?.providers ?? []}
        />
        <BreakdownCard title="By model" rows={breakdowns.data?.models ?? []} />
        <BreakdownCard
          title="By API key"
          rows={breakdowns.data?.api_keys ?? []}
        />
      </div>
    </>
  )
}

function UsageChart({
  title,
  description,
  points,
  field,
  max,
  loading,
  error,
}: {
  title: string
  description: string
  points: JsonRecord[]
  field: string
  max: number
  loading: boolean
  error?: string | { message: string; status?: number }
}) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>{title}</CardTitle>
        <CardDescription>{description}</CardDescription>
      </CardHeader>
      <CardContent>
        <DataState
          loading={loading}
          error={error}
          empty={points.length === 0}
          emptyTitle="No usage in this range"
          emptyDescription="The chart will populate after requests are recorded."
        >
          <div className="flex h-44 items-end gap-1 border-b">
            {points
              .slice()
              .reverse()
              .map((point, index) => (
                <div
                  key={`${String(point.time)}-${index}`}
                  className="min-w-1 flex-1 rounded-t bg-primary/80"
                  title={`${String(point.time)}: ${String(point[field] ?? 0)}`}
                  style={{
                    height: `${Math.max(3, (Number(point[field] ?? 0) / max) * 100)}%`,
                  }}
                />
              ))}
          </div>
        </DataState>
      </CardContent>
    </Card>
  )
}

function BreakdownTable({
  rows,
  label,
  loading,
  error,
}: {
  rows: JsonRecord[]
  label: string
  loading: boolean
  error?: string | { message: string; status?: number }
}) {
  return (
    <DataState
      loading={loading}
      error={error}
      empty={rows.length === 0}
      emptyTitle={`No ${label.toLowerCase()} usage`}
      emptyDescription="No matching usage events were recorded."
    >
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead>{label}</TableHead>
            <TableHead>Requests</TableHead>
            <TableHead>Input tokens</TableHead>
            <TableHead>Output tokens</TableHead>
            <TableHead>Estimated cost</TableHead>
            <TableHead>Success rate</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {rows.map((row, index) => (
            <TableRow key={String(row.id ?? row.name ?? index)}>
              <TableCell className="font-medium">
                {String(row.name ?? row.id ?? "Unattributed")}
              </TableCell>
              <TableCell>
                {Number(row.requests ?? 0).toLocaleString()}
              </TableCell>
              <TableCell>
                {Number(row.input_tokens ?? 0).toLocaleString()}
              </TableCell>
              <TableCell>
                {Number(row.output_tokens ?? 0).toLocaleString()}
              </TableCell>
              <TableCell>
                ${Number(row.estimated_cost ?? 0).toFixed(4)}
              </TableCell>
              <TableCell>{Number(row.success_rate ?? 0).toFixed(1)}%</TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </DataState>
  )
}

function BreakdownCard({ title, rows }: { title: string; rows: JsonRecord[] }) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>{title}</CardTitle>
      </CardHeader>
      <CardContent className="space-y-3">
        {rows.length === 0 ? (
          <p className="text-sm text-muted-foreground">No measured usage.</p>
        ) : (
          rows.slice(0, 8).map((row, index) => (
            <div
              key={String(row.id ?? row.name ?? index)}
              className="flex items-center justify-between border-b pb-2 text-sm"
            >
              <span className="truncate">
                {String(row.name ?? row.id ?? "Unattributed")}
              </span>
              <span className="ml-3 whitespace-nowrap">
                {Number(row.requests ?? 0).toLocaleString()} · $
                {Number(row.estimated_cost ?? 0).toFixed(4)}
              </span>
            </div>
          ))
        )}
      </CardContent>
    </Card>
  )
}

type BillingOverview = {
  configured: boolean
  plan_name?: string
  billing_cycle?: string
  request_allowance?: number
  token_allowance?: number
  payment_status?: string
  current_requests?: number
  upgrade_url?: string
  manage_url?: string
}

type ManagedBillingStatus = {
  configured: boolean
  access_kind?: "free" | "subscription" | "lifetime"
  tier: "free" | "core" | "pro"
  interval?: "monthly" | "annual"
  status?:
    | "on_trial"
    | "active"
    | "paused"
    | "past_due"
    | "unpaid"
    | "cancelled"
    | "expired"
  renews_at?: string
  ends_at?: string
  usage: {
    routed_tokens_this_month: number
    [key: string]: number
  }
  limits: {
    routed_tokens_per_month: number | null
    projects: number | null
    members: number | null
    active_api_keys: number | null
    providers: number | null
    requests_per_minute: number | null
    history_days: number | null
    fallback_targets: number | null
    mcp_servers: number | null
    budget_rules: number | null
    security_patterns: number | null
  }
  features: Record<string, boolean | string>
  overages: string[]
  allowed_transitions: ("free" | "core" | "pro")[]
}

type BillingCatalog = {
  free: {
    tier: "free"
    limits: ManagedBillingStatus["limits"]
    features: ManagedBillingStatus["features"]
  }
  plans: {
    tier: "core" | "pro"
    interval: "monthly" | "annual"
    price: number
    currency: string
    coming_soon_features: string[]
    profile: {
      limits: ManagedBillingStatus["limits"]
      features: ManagedBillingStatus["features"]
    }
  }[]
}

function formatPlanLimit(value: number | null | undefined) {
  if (value === null) return "Unlimited"
  if (value === undefined) return "—"
  return value.toLocaleString()
}

export function OrganizationBillingPage() {
  const { tenantId, edition, tenantRole, gatewayAdmin } = useGateway()
  const overview = useGatewayData<BillingOverview>(
    `/admin/billing/overview?tenant_id=${tenantId}`
  )
  const invoices = useGatewayData<Page<JsonRecord>>(
    `/admin/billing/invoices?tenant_id=${tenantId}`
  )
  const managed = useGatewayData<ManagedBillingStatus>(
    `/commercial/tenants/${tenantId}/billing/status`
  )
  const catalog = useGatewayData<BillingCatalog>("/commercial/billing/catalog")
  const billing = overview.data
  const [commercialPending, setCommercialPending] = React.useState(false)
  const [planConfirmation, setPlanConfirmation] = React.useState<
    "core" | "pro" | null
  >(null)
  const [interval, setInterval] = React.useState<"monthly" | "annual">(
    managed.data?.interval ?? "monthly"
  )
  const canManage = gatewayAdmin || ["owner", "admin"].includes(tenantRole)
  const routedTokenLimit = managed.data?.limits.routed_tokens_per_month
  const accessKind =
    managed.data?.access_kind ??
    (managed.data?.configured ? "subscription" : "free")
  const lifetimeAccess = accessKind === "lifetime"

  async function openCommercialBilling(
    action: "checkout" | "portal",
    tier?: "core" | "pro"
  ) {
    setCommercialPending(true)
    try {
      const result = await gatewayFetch<{ url: string }>(
        `/commercial/tenants/${tenantId}/billing/${action}`,
        tenantId,
        {
          method: "POST",
          headers: { "content-type": "application/json" },
          body:
            action === "checkout"
              ? JSON.stringify({
                  tier,
                  interval,
                  redirect_url: window.location.href,
                })
              : undefined,
        }
      )
      window.location.assign(result.url)
    } catch (error) {
      toast.error(
        error instanceof Error ? error.message : "Billing action failed"
      )
      setCommercialPending(false)
    }
  }

  async function changeCommercialPlan(tier: "core" | "pro") {
    setCommercialPending(true)
    try {
      await gatewayFetch(
        `/commercial/tenants/${tenantId}/billing/subscription`,
        tenantId,
        {
          method: "PATCH",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({ tier, interval }),
        }
      )
      toast.success("Plan change submitted. Billing will update shortly.")
      setPlanConfirmation(null)
      managed.reload()
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Plan change failed")
    } finally {
      setCommercialPending(false)
    }
  }

  return (
    <>
      <PageHeader
        title="Billing"
        action={
          <div className="flex gap-2">
            {edition === "managed" &&
              accessKind === "subscription" &&
              managed.data?.configured &&
              canManage && (
              <Button
                disabled={commercialPending}
                onClick={() => void openCommercialBilling("portal")}
              >
                Customer portal
              </Button>
            )}
            {!lifetimeAccess && billing?.upgrade_url && (
              <Button
                variant="outline"
                render={<a href={billing.upgrade_url} />}
              >
                Upgrade plan
                <ArrowSquareOutIcon data-icon="inline-end" />
              </Button>
            )}
            {!lifetimeAccess && billing?.manage_url && (
              <Button render={<a href={billing.manage_url} />}>
                Manage plan
                <ArrowSquareOutIcon data-icon="inline-end" />
              </Button>
            )}
          </div>
        }
      />
      {edition === "managed" && (
        <DataState
          loading={managed.loading || catalog.loading}
          error={managed.error ?? catalog.error}
          onRetry={() => {
            managed.reload()
            catalog.reload()
          }}
        >
          <div className="mb-4 flex items-center justify-between gap-4">
            <div>
              <Badge variant="secondary" className="capitalize">
                {managed.data?.tier ?? "free"}
              </Badge>
              {lifetimeAccess && (
                <Badge variant="outline" className="ml-2">
                  Lifetime
                </Badge>
              )}
              <p className="mt-2 text-sm text-muted-foreground">
                {Number(
                  managed.data?.usage.routed_tokens_this_month ?? 0
                ).toLocaleString()}{" "}
                routed tokens this month
                {routedTokenLimit === null
                  ? " · Unlimited plan"
                  : ` of ${Number(routedTokenLimit ?? 0).toLocaleString()}`}
              </p>
              {routedTokenLimit !== null && (
                <div className="mt-2 h-2 w-64 max-w-full overflow-hidden rounded-full bg-muted">
                  <div
                    className="h-full bg-primary"
                    style={{
                      width: `${Math.min(100, ((managed.data?.usage.routed_tokens_this_month ?? 0) / Math.max(1, routedTokenLimit ?? 1)) * 100)}%`,
                    }}
                  />
                </div>
              )}
            </div>
            {lifetimeAccess ? (
              <p className="text-sm text-muted-foreground">
                This organization has permanent access and is not billed.
              </p>
            ) : (
              <div className="flex rounded-md border p-1">
                {(["monthly", "annual"] as const).map((value) => (
                  <Button
                    key={value}
                    size="sm"
                    variant={interval === value ? "secondary" : "ghost"}
                    onClick={() => setInterval(value)}
                    className="capitalize"
                  >
                    {value}
                  </Button>
                ))}
              </div>
            )}
          </div>
          {Boolean(managed.data?.overages.length) && (
            <Alert className="mb-4">
              <AlertTitle>Downgrade requires cleanup</AlertTitle>
              <AlertDescription>
                Reduce these resources first:{" "}
                {managed.data?.overages.join(", ")}.
              </AlertDescription>
            </Alert>
          )}
          <div className="mb-6 grid gap-4 lg:grid-cols-3">
            {(["free", "core", "pro"] as const).map((tier) => {
              const paid = catalog.data?.plans.find(
                (entry) => entry.tier === tier && entry.interval === interval
              )
              const profile =
                tier === "free" ? catalog.data?.free : paid?.profile
              const selected =
                managed.data?.tier === tier &&
                (tier === "free" ||
                  lifetimeAccess ||
                  managed.data?.interval === interval)
              const price = paid
                ? new Intl.NumberFormat(undefined, {
                    style: "currency",
                    currency: paid.currency,
                    maximumFractionDigits: 0,
                  }).format(paid.price / 100)
                : "$0"
              return (
                <Card
                  key={tier}
                  className={selected ? "border-primary" : undefined}
                >
                  <CardHeader>
                    <CardTitle className="capitalize">{tier}</CardTitle>
                    <CardDescription>
                      {price}
                      {tier === "free"
                        ? " forever"
                        : interval === "monthly"
                          ? " / month"
                          : " / year"}
                    </CardDescription>
                  </CardHeader>
                  <CardContent>
                    <ul className="mb-5 space-y-1 text-sm text-muted-foreground">
                      <li>
                        {formatPlanLimit(profile?.limits.projects)} projects ·{" "}
                        {formatPlanLimit(profile?.limits.members)} seats
                      </li>
                      <li>
                        {formatPlanLimit(profile?.limits.active_api_keys)}{" "}
                        active API key devices / credentials ·{" "}
                        {formatPlanLimit(profile?.limits.providers)} providers
                      </li>
                      <li>
                        {formatPlanLimit(
                          profile?.limits.routed_tokens_per_month
                        )}{" "}
                        routed tokens / month
                      </li>
                      <li>
                        {formatPlanLimit(profile?.limits.requests_per_minute)}{" "}
                        requests / minute ·{" "}
                        {formatPlanLimit(profile?.limits.history_days)}-day
                        history
                      </li>
                      <li>
                        {formatPlanLimit(profile?.limits.mcp_servers)} MCP
                        servers ·{" "}
                        {formatPlanLimit(profile?.limits.fallback_targets)}{" "}
                        fallbacks
                      </li>
                      <li>
                        {formatPlanLimit(profile?.limits.budget_rules)} budget /
                        quota rules ·{" "}
                        {formatPlanLimit(profile?.limits.security_patterns)}{" "}
                        security patterns
                      </li>
                      {paid?.coming_soon_features.includes("custom_domain") && (
                        <li className="flex items-center gap-2">
                          Custom Domain
                          <Badge variant="outline">Coming Soon</Badge>
                        </li>
                      )}
                    </ul>
                    {selected ? (
                      <Badge variant="secondary">
                        {lifetimeAccess ? "Lifetime plan" : "Current plan"}
                      </Badge>
                    ) : lifetimeAccess && tier !== "free" ? (
                      <p className="text-xs text-muted-foreground">
                        Lifetime plans are managed by the Tuenel operator.
                      </p>
                    ) : tier !== "free" && canManage ? (
                      <Button
                        variant={tier === "pro" ? "default" : "outline"}
                        disabled={commercialPending}
                        onClick={() =>
                          void (managed.data?.configured
                            ? setPlanConfirmation(tier)
                            : openCommercialBilling("checkout", tier))
                        }
                      >
                        {managed.data?.configured
                          ? `Change to ${tier}`
                          : `Choose ${tier}`}
                      </Button>
                    ) : tier !== "free" ? (
                      <p className="text-xs text-muted-foreground">
                        An owner or admin can change this plan.
                      </p>
                    ) : null}
                  </CardContent>
                </Card>
              )
            })}
          </div>
          <Dialog
            open={planConfirmation !== null}
            onOpenChange={(open) => {
              if (!open && !commercialPending) setPlanConfirmation(null)
            }}
          >
            <DialogContent>
              <DialogHeader>
                <DialogTitle>Confirm plan change</DialogTitle>
                <DialogDescription>
                  Change from {managed.data?.tier ?? "free"} to{" "}
                  {planConfirmation ?? "the selected plan"} on the {interval}{" "}
                  billing interval. Lemon Squeezy will apply its standard
                  proration and charge or credit the payment method on file.
                </DialogDescription>
              </DialogHeader>
              <div className="rounded-lg border bg-muted/30 p-4 text-sm">
                This submits the subscription change immediately. You can review
                invoices and payment details in the customer portal.
              </div>
              <DialogFooter>
                <Button
                  variant="outline"
                  disabled={commercialPending}
                  onClick={() => setPlanConfirmation(null)}
                >
                  Cancel
                </Button>
                <Button
                  disabled={commercialPending || planConfirmation === null}
                  onClick={() => {
                    if (planConfirmation)
                      void changeCommercialPlan(planConfirmation)
                  }}
                >
                  {commercialPending ? "Changing plan…" : "Confirm change"}
                </Button>
              </DialogFooter>
            </DialogContent>
          </Dialog>
        </DataState>
      )}
      {edition !== "managed" && (
        <>
          <DataState
            loading={overview.loading}
            error={overview.error}
            onRetry={overview.reload}
          >
            {!billing?.configured && (
              <Alert className="mb-4">
                <AlertTitle>Billing is not configured</AlertTitle>
                <AlertDescription>
                  Connect a billing system or provision provider-neutral billing
                  records. The complete billing layout remains available below.
                </AlertDescription>
              </Alert>
            )}
            <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
              <Metric
                label="Current plan"
                value={billing?.plan_name ?? "Not configured"}
                detail={billing?.billing_cycle ?? "No billing cycle"}
              />
              <Metric
                label="Request allowance"
                value={Number(billing?.request_allowance ?? 0).toLocaleString()}
                detail={`${Number(billing?.current_requests ?? 0).toLocaleString()} consumed`}
              />
              <Metric
                label="Token allowance"
                value={Number(billing?.token_allowance ?? 0).toLocaleString()}
                detail="Current billing cycle"
              />
              <Metric
                label="Payment status"
                value={billing?.payment_status ?? "Unavailable"}
                detail="Provider-neutral state"
              />
            </div>
          </DataState>
          <Card className="mt-6">
            <CardHeader>
              <CardTitle>Invoice history</CardTitle>
              <CardDescription>
                Durable invoices associated with this organization.
              </CardDescription>
            </CardHeader>
            <CardContent>
              <DataState
                loading={invoices.loading}
                error={invoices.error}
                empty={invoices.data?.data.length === 0}
                onRetry={invoices.reload}
                emptyTitle="No invoices"
                emptyDescription="Invoices will appear after billing is configured."
              >
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>Invoice</TableHead>
                      <TableHead>Period</TableHead>
                      <TableHead>Amount</TableHead>
                      <TableHead>Status</TableHead>
                      <TableHead>Issued</TableHead>
                      <TableHead />
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {invoices.data?.data.map((invoice) => (
                      <TableRow key={String(invoice.id)}>
                        <TableCell>
                          {String(invoice.number ?? invoice.id)}
                        </TableCell>
                        <TableCell>
                          {String(invoice.period_start ?? "—")} –{" "}
                          {String(invoice.period_end ?? "—")}
                        </TableCell>
                        <TableCell>
                          {String(invoice.currency ?? "USD")}{" "}
                          {Number(invoice.amount ?? 0).toFixed(2)}
                        </TableCell>
                        <TableCell>
                          <StatusBadge
                            status={String(invoice.status ?? "open")}
                          />
                        </TableCell>
                        <TableCell>
                          {invoice.issued_at
                            ? new Date(
                                String(invoice.issued_at)
                              ).toLocaleDateString()
                            : "—"}
                        </TableCell>
                        <TableCell>
                          {invoice.url ? (
                            <Button
                              variant="ghost"
                              size="sm"
                              render={<a href={String(invoice.url)} />}
                            >
                              Open
                            </Button>
                          ) : null}
                        </TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              </DataState>
            </CardContent>
          </Card>
        </>
      )}
    </>
  )
}

type Organization = {
  id: string
  name: string
  slug: string
  default_environment: string
  region: string
  default_member_role: string
  default_provider_id?: string
  version: number
}

export function OrganizationSettingsPage() {
  const session = useGateway()
  const router = useRouter()
  const locale = usePathname().split("/")[1]
  const organization = useGatewayData<Organization>(
    `/auth/tenants/${session.tenantId}`
  )
  const providers = useGatewayData<Page<Provider>>(
    `/admin/providers?tenant_id=${session.tenantId}`
  )
  const [pending, setPending] = React.useState(false)
  const [danger, setDanger] = React.useState<"leave" | "delete" | null>(null)
  const [confirmation, setConfirmation] = React.useState("")
  const canWrite = ["owner", "admin"].includes(session.tenantRole)

  async function save(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const form = new FormData(event.currentTarget)
    setPending(true)
    try {
      await gatewayFetch(
        `/auth/tenants/${session.tenantId}`,
        session.tenantId,
        {
          method: "PATCH",
          headers: {
            "content-type": "application/json",
            "if-match": `"${organization.data?.version ?? 1}"`,
          },
          body: JSON.stringify({
            name: form.get("name"),
            slug: form.get("slug"),
            default_environment: form.get("default_environment"),
            region: form.get("region"),
            default_member_role: form.get("default_member_role"),
            default_provider_id:
              form.get("default_provider_id") === "none"
                ? null
                : form.get("default_provider_id"),
          }),
        }
      )
      organization.reload()
      router.refresh()
      toast.success("Organization settings saved")
    } catch (error) {
      toast.error(
        error instanceof Error ? error.message : "Settings update failed"
      )
    } finally {
      setPending(false)
    }
  }

  async function destructiveAction() {
    const deleting = danger === "delete"
    setPending(true)
    try {
      await gatewayFetch(
        deleting
          ? `/auth/tenants/${session.tenantId}`
          : `/auth/tenants/${session.tenantId}/members/${session.userId}`,
        session.tenantId,
        {
          method: "DELETE",
          headers: deleting
            ? { "content-type": "application/json" }
            : undefined,
          body: deleting ? JSON.stringify({ confirmation }) : undefined,
        }
      )
      router.replace(`/${locale}`)
      router.refresh()
    } catch (error) {
      toast.error(
        error instanceof Error ? error.message : "Organization action failed"
      )
    } finally {
      setPending(false)
    }
  }

  const value = organization.data
  return (
    <>
      <PageHeader title="Organization Settings" />
      <DataState
        loading={organization.loading}
        error={organization.error}
        onRetry={organization.reload}
      >
        {value && (
          <form onSubmit={save} className="space-y-4">
            <Card>
              <CardHeader>
                <CardTitle>General</CardTitle>
                <CardDescription>
                  Organization identity used throughout the console.
                </CardDescription>
              </CardHeader>
              <CardContent>
                <FieldGroup>
                  <Field>
                    <FieldLabel htmlFor="organization-name">Name</FieldLabel>
                    <Input
                      id="organization-name"
                      name="name"
                      defaultValue={value.name}
                      required
                      minLength={2}
                      maxLength={100}
                      disabled={!canWrite}
                    />
                  </Field>
                  <Field>
                    <FieldLabel htmlFor="organization-slug">Slug</FieldLabel>
                    <Input
                      id="organization-slug"
                      name="slug"
                      defaultValue={value.slug}
                      required
                      pattern="[a-z0-9]+(?:-[a-z0-9]+)*"
                      minLength={2}
                      maxLength={63}
                      disabled={!canWrite}
                    />
                  </Field>
                </FieldGroup>
              </CardContent>
            </Card>
            <Card>
              <CardHeader>
                <CardTitle>Defaults</CardTitle>
                <CardDescription>
                  Applied when new organization resources are created.
                </CardDescription>
              </CardHeader>
              <CardContent className="grid gap-4 md:grid-cols-2">
                <Field>
                  <FieldLabel>Default environment</FieldLabel>
                  <Select
                    name="default_environment"
                    defaultValue={value.default_environment}
                    disabled={!canWrite}
                  >
                    <SelectTrigger className="w-full">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="production">Production</SelectItem>
                      <SelectItem value="staging">Staging</SelectItem>
                      <SelectItem value="development">Development</SelectItem>
                    </SelectContent>
                  </Select>
                </Field>
                <Field>
                  <FieldLabel>Region</FieldLabel>
                  <Select
                    name="region"
                    defaultValue={value.region}
                    disabled={!canWrite}
                  >
                    <SelectTrigger className="w-full">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="global">Global</SelectItem>
                      <SelectItem value="us">United States</SelectItem>
                      <SelectItem value="eu">European Union</SelectItem>
                      <SelectItem value="apac">Asia Pacific</SelectItem>
                    </SelectContent>
                  </Select>
                </Field>
                <Field>
                  <FieldLabel>Default member role</FieldLabel>
                  <Select
                    name="default_member_role"
                    defaultValue={value.default_member_role}
                    disabled={!canWrite}
                  >
                    <SelectTrigger className="w-full">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="engineer">Engineer</SelectItem>
                      <SelectItem value="viewer">Viewer</SelectItem>
                    </SelectContent>
                  </Select>
                </Field>
                <Field>
                  <FieldLabel>Default provider</FieldLabel>
                  <Select
                    name="default_provider_id"
                    defaultValue={value.default_provider_id ?? "none"}
                    disabled={!canWrite}
                  >
                    <SelectTrigger className="w-full">
                      <SelectValue placeholder="No default provider" />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="none">No default provider</SelectItem>
                      {providers.data?.data.map((provider) => (
                        <SelectItem key={provider.id} value={provider.id}>
                          {provider.name ?? provider.id}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </Field>
              </CardContent>
            </Card>
            {canWrite && (
              <div className="flex justify-end">
                <Button type="submit" disabled={pending}>
                  {pending ? "Saving…" : "Save changes"}
                </Button>
              </div>
            )}
          </form>
        )}
      </DataState>
      {session.capabilities.browserSso && canWrite && (
        <OidcSettings tenantId={session.tenantId} />
      )}
      <Card className="mt-6 border-destructive/40">
        <CardHeader>
          <CardTitle>Danger zone</CardTitle>
          <CardDescription>
            These actions change or permanently remove organization access.
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-3 sm:flex-row">
          <Button variant="outline" onClick={() => setDanger("leave")}>
            Leave organization
          </Button>
          <Button
            variant="destructive"
            disabled={session.tenantRole !== "owner"}
            onClick={() => setDanger("delete")}
          >
            Delete organization
          </Button>
        </CardContent>
      </Card>
      <Dialog
        open={danger !== null}
        onOpenChange={(open) => !open && !pending && setDanger(null)}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>
              {danger === "delete"
                ? "Delete organization"
                : "Leave organization"}
            </DialogTitle>
            <DialogDescription>
              {danger === "delete"
                ? "This permanently deletes the organization and its durable data. Type the organization slug to confirm."
                : "You will immediately lose access. The sole owner cannot leave."}
            </DialogDescription>
          </DialogHeader>
          {danger === "delete" && (
            <Input
              value={confirmation}
              onChange={(event) => setConfirmation(event.target.value)}
              placeholder={value?.slug}
              aria-label="Organization slug confirmation"
            />
          )}
          <DialogFooter>
            <Button
              variant={danger === "delete" ? "destructive" : "default"}
              disabled={
                pending || (danger === "delete" && confirmation !== value?.slug)
              }
              onClick={destructiveAction}
            >
              {pending
                ? "Working…"
                : danger === "delete"
                  ? "Delete permanently"
                  : "Leave organization"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  )
}

type OidcConfiguration = {
  issuer_url: string
  client_id: string
  allowed_domains: string[]
  enabled: boolean
  jit_enabled: boolean
  secret_configured: boolean
}

function OidcSettings({ tenantId }: { tenantId: string }) {
  const configuration = useGatewayData<OidcConfiguration>(
    `/commercial/tenants/${tenantId}/oidc`
  )
  const [pending, setPending] = React.useState(false)

  async function save(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const form = new FormData(event.currentTarget)
    setPending(true)
    try {
      await gatewayFetch(`/commercial/tenants/${tenantId}/oidc`, tenantId, {
        method: "PUT",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          issuer_url: form.get("issuer_url"),
          client_id: form.get("client_id"),
          client_secret: form.get("client_secret"),
          allowed_domains: String(form.get("allowed_domains") ?? "")
            .split(",")
            .map((domain) => domain.trim())
            .filter(Boolean),
          enabled: form.get("enabled") === "on",
          jit_enabled: form.get("jit_enabled") === "on",
        }),
      })
      configuration.reload()
      event.currentTarget.reset()
      toast.success("Browser SSO configuration saved")
    } catch (error) {
      toast.error(
        error instanceof Error ? error.message : "SSO configuration failed"
      )
    } finally {
      setPending(false)
    }
  }

  return (
    <Card className="mt-6">
      <CardHeader>
        <CardTitle>Browser SSO</CardTitle>
        <CardDescription>
          Configure one OpenID Connect identity provider for this organization.
          New JIT users always start as viewers.
        </CardDescription>
      </CardHeader>
      <CardContent>
        <form onSubmit={save} className="space-y-4">
          <FieldGroup>
            <Field>
              <FieldLabel htmlFor="oidc-issuer">Issuer URL</FieldLabel>
              <Input
                id="oidc-issuer"
                name="issuer_url"
                type="url"
                defaultValue={configuration.data?.issuer_url}
                placeholder="https://id.example.com"
                required
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="oidc-client-id">Client ID</FieldLabel>
              <Input
                id="oidc-client-id"
                name="client_id"
                defaultValue={configuration.data?.client_id}
                required
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="oidc-secret">Client secret</FieldLabel>
              <Input
                id="oidc-secret"
                name="client_secret"
                type="password"
                autoComplete="new-password"
                required
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="oidc-domains">Allowed domains</FieldLabel>
              <Input
                id="oidc-domains"
                name="allowed_domains"
                defaultValue={configuration.data?.allowed_domains.join(", ")}
                placeholder="example.com, subsidiary.example.com"
                required
              />
            </Field>
            <label className="flex items-center gap-2 text-sm">
              <input
                name="enabled"
                type="checkbox"
                defaultChecked={configuration.data?.enabled}
              />
              Enable browser SSO
            </label>
            <label className="flex items-center gap-2 text-sm">
              <input
                name="jit_enabled"
                type="checkbox"
                defaultChecked={configuration.data?.jit_enabled}
              />
              Allow JIT viewer provisioning
            </label>
          </FieldGroup>
          <Button type="submit" disabled={pending}>
            {pending ? "Saving…" : "Save SSO configuration"}
          </Button>
        </form>
      </CardContent>
    </Card>
  )
}
