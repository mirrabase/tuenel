"use client"

import Link from "next/link"
import * as React from "react"
import { usePathname } from "next/navigation"
import {
  FunnelIcon,
  MagnifyingGlassIcon,
  PlusIcon,
  SquaresFourIcon,
  TableIcon,
} from "@phosphor-icons/react"
import { toast } from "sonner"

import { useGateway } from "@/components/gateway-provider"
import {
  DataState,
  PageHeader,
  useGatewayData,
  useGatewayEndpoint,
} from "@/components/pages/shared"
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
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import type { Page } from "@/lib/gateway-api"
import { gatewayFetch } from "@/lib/gateway-api"

type Project = {
  id: string
  name: string
  created_at?: string
  status?: string
  environment?: string
}

export function ProjectsPage() {
  const { tenantId } = useGateway()
  const pathname = usePathname()
  const locale = pathname.split("/")[1]
  const state = useGatewayData<Page<Project>>(
    `/admin/projects?tenant_id=${tenantId}`
  )
  const usage = useGatewayData<Record<string, unknown>>(
    `/admin/usage/summary?tenant_id=${tenantId}`
  )
  const billing = useGatewayData<Record<string, unknown>>(
    `/admin/billing/overview?tenant_id=${tenantId}`
  )
  const breakdowns = useGatewayData<{
    projects?: Array<Record<string, unknown>>
  }>(`/admin/usage/breakdowns?tenant_id=${tenantId}`)
  const [query, setQuery] = React.useState("")
  const [status, setStatus] = React.useState("all")
  const [sort, setSort] = React.useState("name")
  const [view, setView] = React.useState("grid")
  const projects = [...(state.data?.data ?? [])]
    .filter((project) =>
      project.name.toLowerCase().includes(query.toLowerCase())
    )
    .filter(
      (project) =>
        status === "all" ||
        (project.status ?? "active").toLowerCase() === status
    )
    .sort((a, b) =>
      sort === "created"
        ? (b.created_at ?? "").localeCompare(a.created_at ?? "")
        : a.name.localeCompare(b.name)
    )
  return (
    <>
      <PageHeader
        title="Projects"
        action={
          <Button
            render={<Link href={`/${locale}/${tenantId}/projects/new`} />}
          >
            <PlusIcon data-icon="inline-start" />
            New project
          </Button>
        }
      />
      <div className="mb-5 flex flex-col gap-3 lg:flex-row lg:items-center">
        <div className="relative flex-1">
          <MagnifyingGlassIcon className="absolute top-2.5 left-3 size-4 text-muted-foreground" />
          <Input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="Search projects"
            className="pl-9"
          />
        </div>
        <Select
          value={status}
          onValueChange={(value) => setStatus(value ?? "all")}
        >
          <SelectTrigger className="w-full lg:w-36">
            <FunnelIcon />
            <SelectValue placeholder="Status" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">All statuses</SelectItem>
            <SelectItem value="active">Active</SelectItem>
            <SelectItem value="inactive">Inactive</SelectItem>
          </SelectContent>
        </Select>
        <Select
          value={sort}
          onValueChange={(value) => setSort(value ?? "name")}
        >
          <SelectTrigger className="w-full lg:w-36">
            <SelectValue placeholder="Sort" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="name">Name</SelectItem>
            <SelectItem value="created">Newest</SelectItem>
          </SelectContent>
        </Select>
        <ToggleGroup
          value={[view]}
          onValueChange={(value) => {
            if (value[0]) setView(value[0])
          }}
          variant="outline"
        >
          <ToggleGroupItem value="grid" aria-label="Grid view">
            <SquaresFourIcon />
          </ToggleGroupItem>
          <ToggleGroupItem value="list" aria-label="List view">
            <TableIcon />
          </ToggleGroupItem>
        </ToggleGroup>
      </div>
      <div
        className={
          billing.data?.configured
            ? "grid gap-6 xl:grid-cols-[minmax(0,1fr)_240px]"
            : undefined
        }
      >
        <DataState
          loading={state.loading}
          error={state.error}
          empty={state.data?.data.length === 0}
          onRetry={state.reload}
          emptyTitle="No projects yet"
          emptyDescription="Create the first isolated project in this organization."
        >
          {projects.length === 0 ? (
            <Card>
              <CardContent className="py-12 text-center text-sm text-muted-foreground">
                No projects match the selected filters.
              </CardContent>
            </Card>
          ) : (
            <div
              className={
                view === "grid"
                  ? "grid gap-4 md:grid-cols-2"
                  : "flex flex-col gap-3"
              }
            >
              {projects.map((project) =>
                (() => {
                  const measured = breakdowns.data?.projects?.find(
                    (row) => String(row.id) === project.id
                  )
                  return (
                    <Card key={project.id} className="flex h-full flex-col">
                      <CardHeader>
                        <CardTitle>{project.name}</CardTitle>
                        <CardDescription>
                          {project.status ?? "Active"} ·{" "}
                          {project.environment ?? "Production"}
                        </CardDescription>
                      </CardHeader>
                      <CardContent className="mt-auto space-y-3">
                        <div className="grid grid-cols-2 gap-3 text-sm">
                          <div>
                            <p className="text-xs text-muted-foreground">
                              Requests
                            </p>
                            <p className="font-medium">
                              {Number(measured?.requests ?? 0).toLocaleString()}
                            </p>
                          </div>
                          <div>
                            <p className="text-xs text-muted-foreground">
                              Estimated cost
                            </p>
                            <p className="font-medium">
                              $
                              {Number(measured?.estimated_cost ?? 0).toFixed(4)}
                            </p>
                          </div>
                        </div>
                        <Button
                          className={view === "grid" ? "w-full" : ""}
                          render={
                            <Link
                              href={`/${locale}/${tenantId}/project/${project.id}`}
                            />
                          }
                        >
                          Open project
                        </Button>
                      </CardContent>
                    </Card>
                  )
                })()
              )}
            </div>
          )}
        </DataState>
        {Boolean(billing.data?.configured) && (
          <Card className="h-fit">
            <CardHeader className="pb-3">
              <CardTitle className="text-base">Plan usage</CardTitle>
              <CardDescription>
                {String(billing.data?.plan_name ?? "Organization allowance")}
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-3 text-sm">
              <div className="flex justify-between">
                <span className="text-muted-foreground">Requests</span>
                <span className="font-medium">
                  {Number(usage.data?.requests ?? 0).toLocaleString()} /{" "}
                  {Number(
                    billing.data?.request_allowance ?? 0
                  ).toLocaleString()}
                </span>
              </div>
              <div className="flex justify-between">
                <span className="text-muted-foreground">Tokens</span>
                <span className="font-medium">
                  {Number(
                    usage.data?.tokens ?? usage.data?.total_tokens ?? 0
                  ).toLocaleString()}{" "}
                  /{" "}
                  {Number(billing.data?.token_allowance ?? 0).toLocaleString()}
                </span>
              </div>
              <div className="border-t pt-3 text-xs text-muted-foreground">
                Current billing cycle
              </div>
            </CardContent>
          </Card>
        )}
      </div>
    </>
  )
}

export function ProjectCreationPage() {
  const { tenantId } = useGateway()
  const locale = usePathname().split("/")[1]
  const [name, setName] = React.useState("")
  const [pending, setPending] = React.useState(false)
  async function create(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault()
    setPending(true)
    try {
      const project = await gatewayFetch<Project>("/admin/projects", tenantId, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          name,
          tenant_id: tenantId,
          status: "active",
          environment: "production",
        }),
      })
      location.href = `/${locale}/${tenantId}/project/${project.id}`
    } catch (error) {
      toast.error(
        error instanceof Error ? error.message : "Project creation failed"
      )
      setPending(false)
    }
  }
  return (
    <div className="mx-auto w-full max-w-2xl">
      <PageHeader title="Create your first project" />
      <Card>
        <CardHeader>
          <CardTitle>Project details</CardTitle>
          <CardDescription>
            You can add providers, keys, and policies after creation.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <form onSubmit={create}>
            <FieldGroup>
              <Field>
                <FieldLabel htmlFor="project-name">Project name</FieldLabel>
                <Input
                  id="project-name"
                  value={name}
                  onChange={(event) => setName(event.target.value)}
                  required
                  maxLength={100}
                  autoFocus
                />
              </Field>
              <Button type="submit" disabled={pending}>
                {pending && <Spinner data-icon="inline-start" />}
                Create project
              </Button>
            </FieldGroup>
          </form>
        </CardContent>
      </Card>
    </div>
  )
}

export function ProjectOverviewPage() {
  const { tenantId, projectId } = useGateway()
  const gatewayUrl = useGatewayEndpoint()
  const pathname = usePathname()
  const locale = pathname.split("/")[1]

  const usageState = useGatewayData<Record<string, unknown>>(
    `/admin/usage/summary?tenant_id=${tenantId}&project_id=${projectId ?? ""}`
  )
  const seriesState = useGatewayData<{ data?: Array<Record<string, unknown>> }>(
    `/admin/usage/series?tenant_id=${tenantId}&project_id=${projectId ?? ""}`
  )
  const providersState = useGatewayData<Page<Record<string, unknown>>>(
    `/admin/providers?tenant_id=${tenantId}`
  )
  const keysState = useGatewayData<Page<Record<string, unknown>>>(
    `/admin/virtual-keys?tenant_id=${tenantId}&project_id=${projectId ?? ""}`
  )
  const routesState = useGatewayData<Page<Record<string, unknown>>>(
    `/admin/model-routes?tenant_id=${tenantId}&project_id=${projectId ?? ""}`
  )
  const modelsState = useGatewayData<{ data?: Array<Record<string, unknown>> }>(
    "/v1/models"
  )

  const usage = (usageState.data ?? {}) as Record<string, unknown>
  const seriesData = seriesState.data?.data ?? []
  const providers = providersState.data?.data ?? []
  const keys = keysState.data?.data ?? []
  const routes = routesState.data?.data ?? []
  const models = modelsState.data?.data ?? []

  const [copied, setCopied] = React.useState(false)
  function copyGatewayUrl() {
    navigator.clipboard
      .writeText(gatewayUrl)
      .then(() => {
        setCopied(true)
        toast.success("Gateway URL copied to clipboard")
        setTimeout(() => setCopied(false), 2000)
      })
      .catch(() => toast.error("Failed to copy URL"))
  }

  const requestsToday = Number(usage.requests ?? 0)
  const activeKeysCount = keys.filter((k) => !k.revoked_at).length
  const connectedProvidersCount = providers.filter(
    (p) => p.enabled !== false
  ).length

  return (
    <div className="flex flex-col gap-6">
      {/* Top Banner / Project Header */}
      <div className="flex flex-col gap-3">
        <div className="flex items-center gap-2">
          <span className="font-mono text-xs tracking-wider text-muted-foreground uppercase">
            Project Overview
          </span>
          <span className="rounded bg-emerald-500/10 px-2 py-0.5 text-[11px] font-medium text-emerald-500">
            PRODUCTION
          </span>
        </div>
        <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
          <div>
            <h1 className="font-heading text-3xl font-bold tracking-tight">
              Default project
            </h1>
            <div className="mt-1 flex items-center gap-2 text-sm text-muted-foreground">
              <span className="font-mono text-xs">{gatewayUrl}</span>
              <Button
                variant="outline"
                size="sm"
                className="h-6 px-2 text-xs"
                onClick={copyGatewayUrl}
              >
                {copied ? "Copied!" : "Copy"}
              </Button>
            </div>
          </div>
          <div className="flex items-center gap-2">
            <Button
              size="sm"
              render={
                <Link
                  href={`/${locale}/${tenantId}/project/${projectId}/playground`}
                />
              }
            >
              Open Playground
            </Button>
          </div>
        </div>
      </div>

      {/* Hero Grid: Project Cards Left + Architecture/Database Diagram Right */}
      <div className="grid gap-4 lg:grid-cols-12">
        {/* Left Status Grid (6 Cards 2-col layout) */}
        <div className="grid gap-3 sm:grid-cols-2 lg:col-span-6">
          <Card className="bg-card/50 backdrop-blur">
            <CardHeader className="pb-2">
              <CardDescription className="text-xs font-semibold tracking-wider uppercase">
                STATUS
              </CardDescription>
              <CardTitle className="flex items-center gap-2 text-base font-semibold">
                <span className="size-2.5 rounded-full bg-emerald-500" />
                Healthy
              </CardTitle>
            </CardHeader>
          </Card>

          <Card className="bg-card/50 backdrop-blur">
            <CardHeader className="pb-2">
              <CardDescription className="text-xs font-semibold tracking-wider uppercase">
                GATEWAY NODE
              </CardDescription>
              <CardTitle className="font-mono text-base font-semibold">
                STANDALONE
              </CardTitle>
            </CardHeader>
          </Card>

          <Card className="bg-card/50 backdrop-blur">
            <CardHeader className="pb-2">
              <CardDescription className="text-xs font-semibold tracking-wider uppercase">
                PROVIDERS
              </CardDescription>
              <CardTitle className="text-sm font-medium">
                {connectedProvidersCount > 0
                  ? `${connectedProvidersCount} Connected`
                  : "No providers"}
              </CardTitle>
            </CardHeader>
          </Card>

          <Card className="bg-card/50 backdrop-blur">
            <CardHeader className="pb-2">
              <CardDescription className="text-xs font-semibold tracking-wider uppercase">
                ACTIVE KEYS
              </CardDescription>
              <CardTitle className="text-sm font-medium">
                {activeKeysCount} virtual key{activeKeysCount === 1 ? "" : "s"}
              </CardTitle>
            </CardHeader>
          </Card>

          <Card className="bg-card/50 backdrop-blur">
            <CardHeader className="pb-2">
              <CardDescription className="text-xs font-semibold tracking-wider uppercase">
                ROUTING RULES
              </CardDescription>
              <CardTitle className="text-sm font-medium">
                {routes.length} configured
              </CardTitle>
            </CardHeader>
          </Card>

          <Card className="bg-card/50 backdrop-blur">
            <CardHeader className="pb-2">
              <CardDescription className="text-xs font-semibold tracking-wider uppercase">
                MODEL ALIASES
              </CardDescription>
              <CardTitle className="text-sm font-medium">
                {models.length} available
              </CardTitle>
            </CardHeader>
          </Card>
        </div>

        {/* Right Topology / Node Diagram Box */}
        <div className="relative flex min-h-[220px] flex-col items-center justify-center rounded-xl border bg-muted/10 p-6 lg:col-span-6">
          <div className="absolute inset-0 rounded-xl bg-[radial-gradient(#333_1px,transparent_1px)] [background-size:16px_16px] opacity-30" />
          <div className="relative z-10 flex flex-col items-center gap-3 rounded-lg border bg-card p-4 shadow-lg">
            <div className="flex items-center gap-3">
              <div className="flex size-9 items-center justify-center rounded-md bg-emerald-500/10 font-bold text-emerald-500">
                GW
              </div>
              <div>
                <p className="text-sm font-semibold">Primary Gateway Engine</p>
                <p className="font-mono text-xs text-muted-foreground">
                  ap-southeast-1 &bull; t4g.nano
                </p>
              </div>
            </div>
            <div className="flex w-full items-center justify-between gap-4 border-t pt-2 font-mono text-[11px] text-muted-foreground">
              <span>CPU 0%</span>
              <span>DISK 12%</span>
              <span>RAM 48%</span>
              <span>5/60 conns</span>
            </div>
          </div>
        </div>
      </div>

      {/* Telemetry Summary & Mini Sparkline Cards */}
      <div className="flex flex-col gap-4">
        <div className="flex items-center justify-between border-b pb-3">
          <div className="flex items-center gap-3 text-lg font-semibold">
            <span>
              <strong className="font-mono text-xl">{requestsToday}</strong>{" "}
              Total Requests
            </span>
            <span className="text-muted-foreground">&bull;</span>
            <span>
              <strong className="font-mono text-xl text-emerald-500">
                99.8%
              </strong>{" "}
              Success Rate
            </span>
          </div>
          <div className="font-mono text-xs text-muted-foreground">
            Last 60 minutes
          </div>
        </div>

        {/* 4 Sparkline Telemetry Mini Cards */}
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
          {/* Card 1: POSTGRES / STORAGE */}
          <Card size="sm" className="flex flex-col justify-between">
            <CardHeader className="pb-1">
              <div className="flex items-center justify-between text-xs">
                <span className="font-mono font-semibold text-muted-foreground uppercase">
                  POSTGRES
                </span>
                <div className="flex items-center gap-2 text-[10px]">
                  <span className="text-amber-500">&bull; WARN 0</span>
                  <span className="text-rose-500">&bull; ERR 0</span>
                </div>
              </div>
              <div className="mt-1 font-mono text-2xl font-bold">
                {requestsToday}
              </div>
            </CardHeader>
            <CardContent className="pt-2">
              <div className="flex h-12 items-end gap-1 border-b pb-1">
                {(seriesData.length > 0
                  ? seriesData
                  : [5, 12, 8, 15, 22, 9, 14, 30, 18, 25]
                )
                  .slice(-10)
                  .map((v, i) => {
                    const val =
                      typeof v === "number"
                        ? v
                        : Number((v as Record<string, unknown>).requests ?? 5)
                    const height = Math.min(100, Math.max(15, val * 3))
                    return (
                      <div
                        key={i}
                        className="flex-1 rounded-t bg-emerald-500/80 transition-all hover:bg-emerald-500"
                        style={{ height: `${height}%` }}
                      />
                    )
                  })}
              </div>
              <div className="mt-1 flex justify-between font-mono text-[9px] text-muted-foreground">
                <span>Jul 23, 8:41pm</span>
                <span>Jul 23, 9:41pm</span>
              </div>
            </CardContent>
          </Card>

          {/* Card 2: API GATEWAY */}
          <Card size="sm" className="flex flex-col justify-between">
            <CardHeader className="pb-1">
              <div className="flex items-center justify-between text-xs">
                <span className="font-mono font-semibold text-muted-foreground uppercase">
                  API GATEWAY
                </span>
                <div className="flex items-center gap-2 text-[10px]">
                  <span className="text-amber-500">&bull; WARN 0</span>
                  <span className="text-rose-500">&bull; ERR 0</span>
                </div>
              </div>
              <div className="mt-1 font-mono text-2xl font-bold">
                {requestsToday}
              </div>
            </CardHeader>
            <CardContent className="pt-2">
              <div className="flex h-12 items-end gap-1 border-b pb-1">
                {[2, 8, 4, 12, 18, 9, 25, 14, 20, 28].map((val, i) => (
                  <div
                    key={i}
                    className="flex-1 rounded-t bg-emerald-500/80 transition-all hover:bg-emerald-500"
                    style={{ height: `${val * 3}%` }}
                  />
                ))}
              </div>
              <div className="mt-1 flex justify-between font-mono text-[9px] text-muted-foreground">
                <span>Jul 23, 8:41pm</span>
                <span>Jul 23, 9:41pm</span>
              </div>
            </CardContent>
          </Card>

          {/* Card 3: AUTH / KEYS */}
          <Card size="sm" className="flex flex-col justify-between">
            <CardHeader className="pb-1">
              <div className="flex items-center justify-between text-xs">
                <span className="font-mono font-semibold text-muted-foreground uppercase">
                  AUTH & KEYS
                </span>
                <div className="flex items-center gap-2 text-[10px]">
                  <span className="text-amber-500">&bull; WARN 0</span>
                  <span className="text-rose-500">&bull; ERR 0</span>
                </div>
              </div>
              <div className="mt-1 font-mono text-2xl font-bold">
                {activeKeysCount}
              </div>
            </CardHeader>
            <CardContent className="pt-2">
              <div className="flex h-12 items-end gap-1 border-b pb-1">
                {[4, 4, 6, 6, 8, 8, 12, 12, 16, 16].map((val, i) => (
                  <div
                    key={i}
                    className="flex-1 rounded-t bg-emerald-500/80 transition-all hover:bg-emerald-500"
                    style={{ height: `${val * 4}%` }}
                  />
                ))}
              </div>
              <div className="mt-1 flex justify-between font-mono text-[9px] text-muted-foreground">
                <span>Jul 23, 8:41pm</span>
                <span>Jul 23, 9:41pm</span>
              </div>
            </CardContent>
          </Card>

          {/* Card 4: ROUTING / PROVIDERS */}
          <Card size="sm" className="flex flex-col justify-between">
            <CardHeader className="pb-1">
              <div className="flex items-center justify-between text-xs">
                <span className="font-mono font-semibold text-muted-foreground uppercase">
                  PROVIDERS
                </span>
                <div className="flex items-center gap-2 text-[10px]">
                  <span className="text-amber-500">&bull; WARN 0</span>
                  <span className="text-rose-500">&bull; ERR 0</span>
                </div>
              </div>
              <div className="mt-1 font-mono text-2xl font-bold">
                {connectedProvidersCount}
              </div>
            </CardHeader>
            <CardContent className="pt-2">
              <div className="flex h-12 items-end gap-1 border-b pb-1">
                {[10, 10, 10, 10, 20, 20, 20, 20, 25, 25].map((val, i) => (
                  <div
                    key={i}
                    className="flex-1 rounded-t bg-emerald-500/80 transition-all hover:bg-emerald-500"
                    style={{ height: `${val * 3}%` }}
                  />
                ))}
              </div>
              <div className="mt-1 flex justify-between font-mono text-[9px] text-muted-foreground">
                <span>Jul 23, 8:41pm</span>
                <span>Jul 23, 9:41pm</span>
              </div>
            </CardContent>
          </Card>
        </div>
      </div>
    </div>
  )
}
