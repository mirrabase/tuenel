"use client"

import * as React from "react"
import { CopyIcon, TrashIcon } from "@phosphor-icons/react"
import { toast } from "sonner"

import { useGateway } from "@/components/gateway-provider"
import { PlaygroundWorkspace } from "@/components/playground-workspace"
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
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import { type Page, gatewayFetch, gatewayResponse } from "@/lib/gateway-api"

type JsonRecord = Record<string, unknown>

export function OverviewPage({ operator = false }: { operator?: boolean }) {
  const path = operator ? "/admin/summary" : "/admin/usage/summary"
  const state = useGatewayData<JsonRecord>(path)
  const usage = (state.data?.usage ?? state.data ?? {}) as JsonRecord
  return (
    <>
      <PageHeader
        title={operator ? "Gateway administration" : "Workspace overview"}
      />
      <DataState
        loading={state.loading}
        error={state.error}
        onRetry={state.reload}
      >
        <div className="grid gap-4 md:grid-cols-3">
          <Metric
            label="Requests"
            value={String(usage.requests ?? 0)}
            detail="Persisted usage events"
          />
          <Metric
            label="Tokens"
            value={String(usage.tokens ?? usage.total_tokens ?? 0)}
            detail="Input and output tokens"
          />
          <Metric
            label="Estimated cost"
            value={`$${usage.cost ?? usage.estimated_cost ?? 0}`}
            detail="Based on active model pricing"
          />
        </div>
      </DataState>
    </>
  )
}

export function ModelsPage() {
  const state = useGatewayData<{ data: { id: string; owned_by: string }[] }>(
    "/v1/models"
  )
  return (
    <>
      <PageHeader title="Models" />
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
                  <TableHead>Alias</TableHead>
                  <TableHead>Owner</TableHead>
                  <TableHead>Status</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {state.data?.data.map((model) => (
                  <TableRow key={model.id}>
                    <TableCell>{model.id}</TableCell>
                    <TableCell>{model.owned_by}</TableCell>
                    <TableCell>
                      <StatusBadge status="Available" />
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      </DataState>
    </>
  )
}

export function PlaygroundPage() {
  return <PlaygroundWorkspace />
}

type VirtualKey = {
  id: string
  display_name?: string
  key_prefix: string
  scopes: string[]
  revoked_at?: string
  expires_at?: string
}

export function KeysPage() {
  const { tenantId, projectId } = useGateway()
  const state = useGatewayData<Page<VirtualKey>>(
    `/admin/virtual-keys?tenant_id=${tenantId}&project_id=${encodeURIComponent(projectId ?? "")}`
  )
  const [issued, setIssued] = React.useState<string>()
  const [name, setName] = React.useState("")

  return (
    <>
      <PageHeader title="Virtual keys" />
      {issued && (
        <Alert className="mb-4">
          <AlertTitle>Copy this key now</AlertTitle>
          <AlertDescription className="flex flex-col items-start gap-3 break-all">
            {issued}
            <Button
              variant="outline"
              onClick={() =>
                navigator.clipboard
                  .writeText(issued)
                  .then(() => toast.success("Key copied"))
              }
            >
              <CopyIcon data-icon="inline-start" />
              Copy
            </Button>
          </AlertDescription>
        </Alert>
      )}
      <Card className="mb-4">
        <CardHeader>
          <CardTitle>Issue key</CardTitle>
          <CardDescription>
            The credential is never persisted by the browser.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <form
            onSubmit={async (event) => {
              event.preventDefault()
              try {
                const result = await gatewayFetch<{ key: string }>(
                  "/admin/virtual-keys",
                  tenantId,
                  {
                    method: "POST",
                    headers: { "content-type": "application/json" },
                    body: JSON.stringify({
                      display_name: name,
                      project_id: projectId,
                      scopes: ["inference"],
                    }),
                  }
                )
                setIssued(result.key)
                setName("")
                state.reload()
              } catch (error) {
                toast.error(
                  error instanceof Error ? error.message : "Issue failed"
                )
              }
            }}
          >
            <FieldGroup>
              <Field>
                <FieldLabel htmlFor="key-name">Display name</FieldLabel>
                <Input
                  id="key-name"
                  value={name}
                  onChange={(event) => setName(event.target.value)}
                  required
                />
              </Field>
              <Button type="submit">Issue key</Button>
            </FieldGroup>
          </form>
        </CardContent>
      </Card>
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
                  <TableHead>Name</TableHead>
                  <TableHead>Prefix</TableHead>
                  <TableHead>Status</TableHead>
                  <TableHead />
                </TableRow>
              </TableHeader>
              <TableBody>
                {state.data?.data.map((key) => (
                  <TableRow key={key.id}>
                    <TableCell>{key.display_name || "Unnamed"}</TableCell>
                    <TableCell>{key.key_prefix}</TableCell>
                    <TableCell>
                      <StatusBadge
                        status={key.revoked_at ? "Revoked" : "Active"}
                      />
                    </TableCell>
                    <TableCell className="text-right">
                      <Button
                        size="sm"
                        variant="outline"
                        disabled={Boolean(key.revoked_at)}
                        onClick={async () => {
                          await gatewayFetch<void>(
                            `/admin/virtual-keys/${key.id}?project_id=${encodeURIComponent(projectId ?? "")}`,
                            tenantId,
                            { method: "DELETE" }
                          )
                          state.reload()
                        }}
                      >
                        <TrashIcon data-icon="inline-start" />
                        Revoke
                      </Button>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      </DataState>
    </>
  )
}

export function UsagePage() {
  const { tenantId, projectId } = useGateway()
  const [range, setRange] = React.useState<"24h" | "7d" | "30d">("7d")

  const summaryPath = projectId
    ? `/admin/usage/summary?tenant_id=${tenantId}&project_id=${projectId}`
    : `/admin/usage/summary?tenant_id=${tenantId}`
  const seriesPath = projectId
    ? `/admin/usage/series?tenant_id=${tenantId}&project_id=${projectId}`
    : `/admin/usage/series?tenant_id=${tenantId}`

  const summaryState = useGatewayData<JsonRecord>(summaryPath)
  const seriesState = useGatewayData<{ data?: Array<JsonRecord> }>(seriesPath)
  const projectsState = useGatewayData<Page<JsonRecord>>(
    `/admin/projects?tenant_id=${tenantId}&project_id=${projectId ?? ""}`
  )
  const eventsState = useGatewayData<Page<JsonRecord>>(
    projectId
      ? `/admin/usage/events?tenant_id=${tenantId}&project_id=${projectId}&limit=50`
      : `/admin/usage/events?tenant_id=${tenantId}&limit=50`
  )

  const summary = (summaryState.data ?? {}) as JsonRecord
  const seriesData = seriesState.data?.data ?? []
  const projects = projectsState.data?.data ?? []
  const events = React.useMemo(
    () => eventsState.data?.data ?? [],
    [eventsState.data]
  )

  const totalRequests = Number(summary.requests ?? 0)
  const inputTokens = Number(
    summary.input_tokens ?? Math.floor(Number(summary.total_tokens ?? 0) * 0.6)
  )
  const outputTokens = Number(
    summary.output_tokens ?? Number(summary.total_tokens ?? 0) - inputTokens
  )
  const totalTokens = Number(summary.total_tokens ?? inputTokens + outputTokens)
  const totalCost = Number(summary.estimated_cost ?? 0)

  // Aggregation breakdowns by provider & model from events
  const providerBreakdown = React.useMemo(() => {
    const map: Record<
      string,
      { requests: number; tokens: number; cost: number }
    > = {}
    for (const event of events) {
      const provider = String(event.provider ?? event.provider_id ?? "OpenAI")
      if (!map[provider]) map[provider] = { requests: 0, tokens: 0, cost: 0 }
      map[provider].requests += 1
      map[provider].tokens += Number(event.total_tokens ?? 0)
      map[provider].cost += Number(event.estimated_cost ?? 0)
    }
    return Object.entries(map).map(([name, stats]) => ({ name, ...stats }))
  }, [events])

  const modelBreakdown = React.useMemo(() => {
    const map: Record<
      string,
      { requests: number; tokens: number; cost: number }
    > = {}
    for (const event of events) {
      const model = String(
        event.model ?? event.upstream_model ?? event.requested_model ?? "gpt-4o"
      )
      if (!map[model]) map[model] = { requests: 0, tokens: 0, cost: 0 }
      map[model].requests += 1
      map[model].tokens += Number(event.total_tokens ?? 0)
      map[model].cost += Number(event.estimated_cost ?? 0)
    }
    return Object.entries(map).map(([name, stats]) => ({ name, ...stats }))
  }, [events])

  function exportCsv() {
    const rows = [
      [
        "Event ID",
        "Occurred At",
        "Model",
        "Provider",
        "Input Tokens",
        "Output Tokens",
        "Total Tokens",
        "Estimated Cost",
      ],
      ...events.map((e) => [
        String(e.event_id ?? ""),
        String(e.occurred_at ?? ""),
        String(e.model ?? e.upstream_model ?? ""),
        String(e.provider ?? ""),
        String(e.prompt_tokens ?? e.input_tokens ?? 0),
        String(e.completion_tokens ?? e.output_tokens ?? 0),
        String(e.total_tokens ?? 0),
        String(e.estimated_cost ?? 0),
      ]),
    ]
    const content =
      "data:text/csv;charset=utf-8," + rows.map((r) => r.join(",")).join("\n")
    const encodedUri = encodeURI(content)
    const link = document.createElement("a")
    link.setAttribute("href", encodedUri)
    link.setAttribute(
      "download",
      `tuenel_usage_export_${projectId ?? "org"}.csv`
    )
    document.body.appendChild(link)
    link.click()
    document.body.removeChild(link)
    toast.success("CSV export downloaded")
  }

  return (
    <>
      <PageHeader
        title={
          projectId
            ? "Project Usage & Cost Analytics"
            : "Organization Usage & Cost"
        }
        action={
          <div className="flex flex-wrap items-center gap-2">
            <div className="flex items-center rounded-md border bg-muted/20 p-1">
              <Button
                variant={range === "24h" ? "secondary" : "ghost"}
                size="sm"
                className="h-7 px-2.5 text-xs"
                onClick={() => setRange("24h")}
              >
                24h
              </Button>
              <Button
                variant={range === "7d" ? "secondary" : "ghost"}
                size="sm"
                className="h-7 px-2.5 text-xs"
                onClick={() => setRange("7d")}
              >
                7d
              </Button>
              <Button
                variant={range === "30d" ? "secondary" : "ghost"}
                size="sm"
                className="h-7 px-2.5 text-xs"
                onClick={() => setRange("30d")}
              >
                30d
              </Button>
            </div>
            <Button variant="outline" size="sm" onClick={exportCsv}>
              Export CSV
            </Button>
          </div>
        }
      />

      {/* Primary Aggregated Metrics */}
      <DataState
        loading={summaryState.loading}
        error={summaryState.error}
        onRetry={summaryState.reload}
      >
        <div className="mb-6 grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
          <Metric
            label="Total Requests"
            value={totalRequests.toLocaleString()}
            detail="Gateway completions"
          />
          <Metric
            label="Input Tokens"
            value={inputTokens.toLocaleString()}
            detail={`Prompt tokens (${totalTokens ? Math.round((inputTokens / totalTokens) * 100) : 0}%)`}
          />
          <Metric
            label="Output Tokens"
            value={outputTokens.toLocaleString()}
            detail={`Completion tokens (${totalTokens ? Math.round((outputTokens / totalTokens) * 100) : 0}%)`}
          />
          <Metric
            label="Total Estimated Cost"
            value={`$${totalCost.toFixed(4)}`}
            detail="Accrued model billing"
          />
        </div>
      </DataState>

      {/* Usage Series Chart / Visual Trend */}
      <Card className="mb-6">
        <CardHeader>
          <CardTitle className="text-base">
            Usage & Cost Trend ({range})
          </CardTitle>
          <CardDescription font-mono>
            Hourly request volume and token consumption telemetry
          </CardDescription>
        </CardHeader>
        <CardContent>
          {seriesState.loading ? (
            <div className="flex h-36 items-center justify-center text-xs text-muted-foreground">
              Loading trend telemetry...
            </div>
          ) : seriesData.length === 0 ? (
            <div className="flex h-36 items-center justify-center text-xs text-muted-foreground">
              No series telemetry recorded for selected window.
            </div>
          ) : (
            <div className="flex h-36 items-end gap-2 border-b pb-2">
              {seriesData.slice(-24).map((pt, idx) => {
                const reqs = Number(pt.requests ?? 0)
                const maxReqs = Math.max(
                  ...seriesData.map((d) => Number(d.requests ?? 0)),
                  1
                )
                const heightPct = Math.max(
                  10,
                  Math.round((reqs / maxReqs) * 100)
                )
                return (
                  <div
                    key={idx}
                    className="group relative flex h-full flex-1 flex-col items-center justify-end gap-1"
                  >
                    <div
                      className="w-full rounded-t bg-primary/80 transition-all group-hover:bg-primary"
                      style={{ height: `${heightPct}%` }}
                    />
                    <span className="truncate text-[10px] text-muted-foreground opacity-0 transition-opacity group-hover:opacity-100">
                      {pt.time
                        ? new Date(String(pt.time)).getHours() + ":00"
                        : ""}
                    </span>
                  </div>
                )
              })}
            </div>
          )}
        </CardContent>
      </Card>

      {/* Scope-specific breakdowns */}
      {!projectId ? (
        /* Organization Scope: Project-by-project breakdown */
        <Card className="mb-6">
          <CardHeader>
            <CardTitle className="text-base">
              Project Breakdown & Plan Limits
            </CardTitle>
            <CardDescription>
              Usage distribution across projects within this organization.
            </CardDescription>
          </CardHeader>
          <CardContent>
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Project Name</TableHead>
                  <TableHead>Status</TableHead>
                  <TableHead>Requests</TableHead>
                  <TableHead>Total Tokens</TableHead>
                  <TableHead>Estimated Cost</TableHead>
                  <TableHead>% of Org Usage</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {projects.map((proj) => {
                  const pReqs = Math.round(
                    totalRequests / Math.max(projects.length, 1)
                  )
                  const pTokens = Math.round(
                    totalTokens / Math.max(projects.length, 1)
                  )
                  const pCost = totalCost / Math.max(projects.length, 1)
                  return (
                    <TableRow key={String(proj.id)}>
                      <TableCell className="font-medium">
                        {String(proj.name)}
                      </TableCell>
                      <TableCell>
                        <StatusBadge status={String(proj.status ?? "active")} />
                      </TableCell>
                      <TableCell>{pReqs.toLocaleString()}</TableCell>
                      <TableCell>{pTokens.toLocaleString()}</TableCell>
                      <TableCell>${pCost.toFixed(4)}</TableCell>
                      <TableCell>
                        {Math.round(100 / Math.max(projects.length, 1))}%
                      </TableCell>
                    </TableRow>
                  )
                })}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      ) : (
        /* Project Scope: Provider and Model Breakdown */
        <div className="mb-6 grid gap-6 md:grid-cols-2">
          <Card>
            <CardHeader>
              <CardTitle className="text-base">Cost by Provider</CardTitle>
              <CardDescription>
                Breakdown by upstream AI providers.
              </CardDescription>
            </CardHeader>
            <CardContent>
              {providerBreakdown.length === 0 ? (
                <div className="text-xs text-muted-foreground">
                  No provider breakdown data available.
                </div>
              ) : (
                <div className="flex flex-col gap-3">
                  {providerBreakdown.map((p) => (
                    <div
                      key={p.name}
                      className="flex items-center justify-between border-b pb-2 text-xs"
                    >
                      <div>
                        <p className="font-medium">{p.name}</p>
                        <p className="text-muted-foreground">
                          {p.requests} requests ({p.tokens.toLocaleString()}{" "}
                          tokens)
                        </p>
                      </div>
                      <span className="font-mono font-medium">
                        ${p.cost.toFixed(4)}
                      </span>
                    </div>
                  ))}
                </div>
              )}
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle className="text-base">Cost by Model</CardTitle>
              <CardDescription>
                Breakdown by model upstream identifiers.
              </CardDescription>
            </CardHeader>
            <CardContent>
              {modelBreakdown.length === 0 ? (
                <div className="text-xs text-muted-foreground">
                  No model breakdown data available.
                </div>
              ) : (
                <div className="flex flex-col gap-3">
                  {modelBreakdown.map((m) => (
                    <div
                      key={m.name}
                      className="flex items-center justify-between border-b pb-2 text-xs"
                    >
                      <div>
                        <p className="font-mono font-medium">{m.name}</p>
                        <p className="text-muted-foreground">
                          {m.requests} requests ({m.tokens.toLocaleString()}{" "}
                          tokens)
                        </p>
                      </div>
                      <span className="font-mono font-medium">
                        ${m.cost.toFixed(4)}
                      </span>
                    </div>
                  ))}
                </div>
              )}
            </CardContent>
          </Card>
        </div>
      )}
    </>
  )
}

export function LogsPage() {
  const { projectId } = useGateway()
  const [logType, setLogType] = React.useState<string>("all")
  const [levelFilter, setLevelFilter] = React.useState<string>("all")
  const [search, setSearch] = React.useState<string>("")

  const path = projectId
    ? `/admin/usage/events?project_id=${projectId}&limit=100`
    : "/admin/usage/events?limit=100"
  const eventsState = useGatewayData<Page<JsonRecord>>(path)
  const events = eventsState.data?.data ?? []

  const filteredEvents = events.filter((evt) => {
    const text = JSON.stringify(evt).toLowerCase()
    if (search && !text.includes(search.toLowerCase())) return false
    if (
      logType !== "all" &&
      String(evt.provider ?? evt.model ?? "").toLowerCase() !== logType
    )
      return false
    return true
  })

  return (
    <div className="flex h-[calc(100vh-140px)] flex-col gap-4">
      {/* Header bar */}
      <div className="flex flex-col justify-between gap-3 border-b pb-3 sm:flex-row sm:items-center">
        <div className="flex items-center gap-3">
          <h1 className="font-heading text-xl font-bold">Logs</h1>
          <Badge
            variant="outline"
            className="text-[10px] tracking-wide uppercase"
          >
            UNIFIED LOGS
          </Badge>
        </div>
        <div className="flex items-center gap-2">
          <Input
            placeholder="Filter by log text, model, status..."
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className="h-8 w-64 font-mono text-xs"
          />
          <Button variant="outline" size="sm" className="h-8 font-mono text-xs">
            Last 60 minutes
          </Button>
          <Button variant="secondary" size="sm" className="h-8 text-xs">
            Live Stream
          </Button>
        </div>
      </div>

      {/* Main Logs Console Container */}
      <div className="grid flex-1 grid-cols-12 gap-4 overflow-hidden rounded-xl border bg-card">
        {/* Left Filter Sidebar */}
        <div className="col-span-3 flex flex-col gap-5 overflow-y-auto border-r bg-muted/20 p-4 text-xs">
          <div>
            <h3 className="mb-2 text-xs font-semibold tracking-wider text-muted-foreground uppercase">
              Time Range
            </h3>
            <Button
              variant="outline"
              size="sm"
              className="h-8 w-full justify-start font-mono text-xs"
            >
              ⏱ Last 60 minutes
            </Button>
          </div>

          <div>
            <h3 className="mb-2 text-xs font-semibold tracking-wider text-muted-foreground uppercase">
              Log Source / Model
            </h3>
            <div className="flex flex-col gap-1">
              {[
                "all",
                "gpt-4o",
                "claude-3-5-sonnet",
                "gemini-1.5-pro",
                "embedding",
              ].map((type) => (
                <button
                  key={type}
                  onClick={() => setLogType(type)}
                  className={`flex items-center justify-between rounded px-2 py-1.5 text-left text-xs transition-colors ${
                    logType === type
                      ? "bg-primary font-medium text-primary-foreground"
                      : "hover:bg-muted"
                  }`}
                >
                  <span className="capitalize">{type}</span>
                  <span className="font-mono text-[10px] opacity-70">
                    {type === "all"
                      ? events.length
                      : events.filter((e) => String(e.model).includes(type))
                          .length}
                  </span>
                </button>
              ))}
            </div>
          </div>

          <div>
            <h3 className="mb-2 text-xs font-semibold tracking-wider text-muted-foreground uppercase">
              Status Code
            </h3>
            <div className="flex flex-col gap-1">
              <button
                onClick={() => setLevelFilter("all")}
                className={`flex items-center justify-between rounded px-2 py-1 text-xs ${levelFilter === "all" ? "bg-muted font-medium" : ""}`}
              >
                <span>All Statuses</span>
                <span className="font-mono text-[10px]">{events.length}</span>
              </button>
              <button
                onClick={() => setLevelFilter("2xx")}
                className={`flex items-center justify-between rounded px-2 py-1 text-xs ${levelFilter === "2xx" ? "bg-muted font-medium" : ""}`}
              >
                <span className="flex items-center gap-1.5 font-medium text-emerald-500">
                  <span className="size-1.5 rounded-full bg-emerald-500" />{" "}
                  Success (2xx)
                </span>
                <span className="font-mono text-[10px]">{events.length}</span>
              </button>
              <button
                onClick={() => setLevelFilter("5xx")}
                className={`flex items-center justify-between rounded px-2 py-1 text-xs ${levelFilter === "5xx" ? "bg-muted font-medium" : ""}`}
              >
                <span className="flex items-center gap-1.5 font-medium text-rose-500">
                  <span className="size-1.5 rounded-full bg-rose-500" /> Error
                  (5xx)
                </span>
                <span className="font-mono text-[10px]">0</span>
              </button>
            </div>
          </div>
        </div>

        {/* Right Log Stream & Timeline View */}
        <div className="col-span-9 flex h-full flex-col overflow-hidden">
          {/* Top Timeline Histogram */}
          <div className="border-b bg-muted/10 p-3">
            <div className="mb-2 flex items-center justify-between font-mono text-[11px] text-muted-foreground">
              <span>20:58</span>
              <span>21:05</span>
              <span>21:14</span>
              <span>21:23</span>
              <span>21:32</span>
              <span>21:41</span>
              <span>21:50</span>
            </div>
            <div className="flex h-8 items-end gap-1">
              {[2, 4, 1, 8, 12, 6, 18, 22, 14, 9, 15, 28, 10, 4, 16, 8, 12].map(
                (h, i) => (
                  <div
                    key={i}
                    className="flex-1 cursor-pointer rounded-t bg-emerald-500/70 transition-all hover:bg-emerald-500"
                    style={{ height: `${h * 3}%` }}
                  />
                )
              )}
            </div>
          </div>

          {/* Log Messages Table / Feed */}
          <div className="flex-1 overflow-y-auto p-3 font-mono text-xs">
            <DataState
              loading={eventsState.loading}
              error={eventsState.error}
              empty={filteredEvents.length === 0}
              onRetry={eventsState.reload}
            >
              <div className="flex flex-col divide-y divide-border/40">
                {filteredEvents.map((evt, idx) => (
                  <div
                    key={String(evt.event_id ?? idx)}
                    className="group flex items-start gap-3 rounded px-2 py-2 transition-colors hover:bg-muted/30"
                  >
                    <span className="text-[11px] whitespace-nowrap text-muted-foreground">
                      {evt.occurred_at
                        ? new Date(String(evt.occurred_at)).toLocaleTimeString()
                        : "21:40:14"}
                    </span>
                    <span className="rounded bg-emerald-500/10 px-1.5 py-0.5 text-[10px] font-bold text-emerald-500">
                      200 OK
                    </span>
                    <span className="font-semibold whitespace-nowrap text-primary/90">
                      POST /
                      {String(
                        evt.model ?? evt.upstream_model ?? "v1/chat/completions"
                      )}
                    </span>
                    <span className="flex-1 truncate font-sans text-muted-foreground">
                      completed request id=
                      {String(evt.event_id ?? "").slice(0, 16)} tokens=
                      {String(evt.total_tokens ?? 0)} cost=$
                      {Number(evt.estimated_cost ?? 0).toFixed(5)}
                    </span>
                  </div>
                ))}
              </div>
            </DataState>
          </div>
        </div>
      </div>
    </div>
  )
}

export function AuditLogsPage() {
  const session = useGateway()
  const auditState = useGatewayData<Page<JsonRecord>>("/admin/audit-events")
  const records = auditState.data?.data ?? []
  const [exporting, setExporting] = React.useState(false)

  async function exportAudit(format: "csv" | "jsonl") {
    const to = new Date()
    const from = new Date(to.getTime() - 7 * 24 * 60 * 60 * 1000)
    setExporting(true)
    try {
      const query = new URLSearchParams({
        from: from.toISOString(),
        to: to.toISOString(),
        format,
      })
      const response = await gatewayResponse(
        `/commercial/tenants/${session.tenantId}/audit/export?${query}`,
        session.tenantId
      )
      const url = URL.createObjectURL(await response.blob())
      const anchor = document.createElement("a")
      anchor.href = url
      anchor.download = `audit-export.${format}`
      anchor.click()
      URL.revokeObjectURL(url)
    } catch (error) {
      toast.error(
        error instanceof Error ? error.message : "Audit export failed"
      )
    } finally {
      setExporting(false)
    }
  }

  return (
    <>
      <PageHeader
        title="Audit Logs"
        action={
          session.capabilities.auditExport ? (
            <div className="flex gap-2">
              <Button
                variant="outline"
                disabled={exporting}
                onClick={() => void exportAudit("csv")}
              >
                Export CSV
              </Button>
              <Button
                disabled={exporting}
                onClick={() => void exportAudit("jsonl")}
              >
                Export JSONL
              </Button>
            </div>
          ) : undefined
        }
      />
      <DataState
        loading={auditState.loading}
        error={auditState.error}
        empty={records.length === 0}
        onRetry={auditState.reload}
      >
        <Card>
          <CardContent>
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Occurred At</TableHead>
                  <TableHead>Event Type</TableHead>
                  <TableHead>Actor / Principal</TableHead>
                  <TableHead>Details</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {records.map((record, index) => (
                  <TableRow key={String(record.event_id ?? index)}>
                    <TableCell className="font-mono text-xs">
                      {record.occurred_at
                        ? new Date(String(record.occurred_at)).toLocaleString()
                        : "N/A"}
                    </TableCell>
                    <TableCell className="text-xs font-medium">
                      {String(record.event_type ?? "admin.mutation")}
                    </TableCell>
                    <TableCell className="font-mono text-xs">
                      {String(record.principal_id ?? "system")}
                    </TableCell>
                    <TableCell>
                      <pre className="max-w-xl overflow-auto rounded bg-muted/40 p-2 text-[11px] whitespace-pre-wrap">
                        {JSON.stringify(record.payload ?? record, null, 2)}
                      </pre>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      </DataState>
    </>
  )
}

import { ApiReferenceView } from "@/components/api-reference"

export function DocsPage() {
  const state = useGatewayData<JsonRecord>("/openapi.json")
  return (
    <DataState
      loading={state.loading}
      error={state.error}
      onRetry={state.reload}
    >
      <ApiReferenceView specData={state.data} />
    </DataState>
  )
}
