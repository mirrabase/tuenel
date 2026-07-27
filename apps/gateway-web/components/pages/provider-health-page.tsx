"use client"

import Link from "next/link"
import * as React from "react"
import { usePathname } from "next/navigation"
import { ArrowClockwiseIcon } from "@phosphor-icons/react"
import {
  Area,
  AreaChart,
  CartesianGrid,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts"
import { toast } from "sonner"

import { useGateway } from "@/components/gateway-provider"
import {
  DataState,
  Metric,
  PageHeader,
  StatusBadge,
  TimeRangeSelector,
  type TimeRange,
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
import { gatewayFetch } from "@/lib/gateway-api"

type HealthProvider = {
  provider_id: string
  name: string
  current_status: string
  success_rate: number | null
  error_rate: number | null
  p50_latency_ms: number | null
  p95_latency_ms: number | null
  last_successful_request: string | null
  last_failure: string | null
}

type HealthPoint = {
  time: string
  availability: number
  p50_latency_ms: number | null
  p95_latency_ms: number | null
  requests: number
  errors: number
}

type ProviderHealthData = {
  summary: {
    healthy_providers: number
    degraded_providers: number
    average_success_rate: number | null
    average_p95_latency_ms: number | null
  }
  providers: HealthProvider[]
  series: HealthPoint[]
  filters: {
    providers: { id: string; name: string }[]
    models: string[]
  }
}

const rangeHours: Record<TimeRange, number> = {
  "24h": 24,
  "7d": 168,
  "30d": 720,
}

function percentage(value: number | null) {
  return value === null ? "—" : `${Number(value).toFixed(1)}%`
}

function latency(value: number | null) {
  return value === null ? "—" : `${Number(value).toFixed(0)} ms`
}

function timestamp(value: string | null) {
  return value ? new Date(value).toLocaleString() : "Never"
}

function HealthChart({
  title,
  description,
  data,
  lines,
}: {
  title: string
  description: string
  data: HealthPoint[]
  lines: { key: keyof HealthPoint; label: string; color: string }[]
}) {
  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-base">{title}</CardTitle>
        <CardDescription>{description}</CardDescription>
      </CardHeader>
      <CardContent>
        <div className="h-56">
          <ResponsiveContainer width="100%" height="100%">
            <AreaChart data={data}>
              <CartesianGrid strokeDasharray="3 3" vertical={false} />
              <XAxis
                dataKey="time"
                fontSize={11}
                tickFormatter={(value) =>
                  new Date(String(value)).toLocaleDateString(undefined, {
                    month: "short",
                    day: "numeric",
                  })
                }
              />
              <YAxis fontSize={11} width={48} />
              <Tooltip
                labelFormatter={(value) =>
                  new Date(String(value)).toLocaleString()
                }
                contentStyle={{
                  background: "var(--card)",
                  borderColor: "var(--border)",
                  borderRadius: 8,
                }}
              />
              {lines.map((line) => (
                <Area
                  key={line.key}
                  type="monotone"
                  dataKey={line.key}
                  name={line.label}
                  stroke={line.color}
                  fill={line.color}
                  fillOpacity={0.1}
                />
              ))}
            </AreaChart>
          </ResponsiveContainer>
        </div>
        {!data.length && (
          <p className="mt-2 text-center text-xs text-muted-foreground">
            No requests in this range.
          </p>
        )}
      </CardContent>
    </Card>
  )
}

export function ProviderHealthPage() {
  const session = useGateway()
  const pathname = usePathname()
  const locale = pathname.split("/")[1]
  const [range, setRange] = React.useState<TimeRange>("24h")
  const [providerId, setProviderId] = React.useState("")
  const [model, setModel] = React.useState("")
  const [checking, setChecking] = React.useState(false)
  const [now] = React.useState(Date.now)
  const query = React.useMemo(() => {
    const params = new URLSearchParams({
      tenant_id: session.tenantId,
      project_id: session.projectId ?? "",
      from: new Date(now - rangeHours[range] * 3_600_000).toISOString(),
      to: new Date(now).toISOString(),
    })
    if (providerId) params.set("provider_id", providerId)
    if (model) params.set("model", model)
    return `/admin/provider-health?${params}`
  }, [model, now, providerId, range, session.projectId, session.tenantId])
  const health = useGatewayData<ProviderHealthData>(query)
  const summary = health.data?.summary
  const rows = health.data?.providers ?? []
  const points = health.data?.series ?? []

  async function runChecks() {
    const providerIds = rows.map((provider) => provider.provider_id)
    if (!providerIds.length) return
    setChecking(true)
    const results = await Promise.allSettled(
      providerIds.map((id) =>
        gatewayFetch(
          `/admin/providers/${encodeURIComponent(id)}/check?tenant_id=${encodeURIComponent(session.tenantId)}`,
          session.tenantId,
          { method: "POST" }
        )
      )
    )
    const failed = results.filter(
      (result) => result.status === "rejected"
    ).length
    if (failed)
      toast.error(
        `${providerIds.length - failed} checks completed; ${failed} failed`
      )
    else toast.success("Provider health refreshed")
    health.reload()
    setChecking(false)
  }

  return (
    <>
      <PageHeader
        title="Provider Health"
        action={
          <div className="flex flex-wrap gap-2">
            <TimeRangeSelector value={range} onChange={setRange} />
            <Button
              variant="outline"
              disabled={checking || !rows.length}
              onClick={runChecks}
            >
              <ArrowClockwiseIcon data-icon="inline-start" />
              {checking ? "Running…" : "Run health check"}
            </Button>
          </div>
        }
      />

      <div className="mb-6 flex flex-wrap gap-3">
        <Select
          value={providerId || "all"}
          onValueChange={(value) =>
            setProviderId(value && value !== "all" ? value : "")
          }
        >
          <SelectTrigger className="w-56">
            <SelectValue placeholder="All providers" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">All providers</SelectItem>
            {health.data?.filters.providers.map((provider) => (
              <SelectItem key={provider.id} value={provider.id}>
                {provider.name}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        <Select
          value={model || "all"}
          onValueChange={(value) =>
            setModel(value && value !== "all" ? value : "")
          }
        >
          <SelectTrigger className="w-64">
            <SelectValue placeholder="All models" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">All models</SelectItem>
            {health.data?.filters.models.map((option) => (
              <SelectItem key={option} value={option}>
                {option}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>

      <DataState
        loading={health.loading}
        error={health.error}
        onRetry={health.reload}
      >
        <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-4">
          <Metric
            label="Healthy providers"
            value={Number(summary?.healthy_providers ?? 0).toLocaleString()}
            detail="Current status"
          />
          <Metric
            label="Degraded providers"
            value={Number(summary?.degraded_providers ?? 0).toLocaleString()}
            detail="Needs attention"
          />
          <Metric
            label="Average success rate"
            value={percentage(summary?.average_success_rate ?? null)}
            detail={range}
          />
          <Metric
            label="Average p95 latency"
            value={latency(summary?.average_p95_latency_ms ?? null)}
            detail={range}
          />
        </div>

        <Card className="mt-6">
          <CardContent>
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Provider</TableHead>
                  <TableHead>Current status</TableHead>
                  <TableHead>Success rate</TableHead>
                  <TableHead>Error rate</TableHead>
                  <TableHead>p50 latency</TableHead>
                  <TableHead>p95 latency</TableHead>
                  <TableHead>Last successful request</TableHead>
                  <TableHead>Last failure</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {!rows.length && (
                  <TableRow>
                    <TableCell colSpan={8} className="h-36 text-center">
                      <p className="font-medium">No provider health data</p>
                      <p className="mt-1 text-sm text-muted-foreground">
                        Adjust the filters or run traffic through an enabled
                        provider.
                      </p>
                    </TableCell>
                  </TableRow>
                )}
                {rows.map((provider) => (
                  <TableRow key={provider.provider_id}>
                    <TableCell className="font-medium">
                      <Link
                        className="hover:underline"
                        href={`/${locale}/${session.tenantId}/project/${session.projectId}/providers?provider=${encodeURIComponent(provider.provider_id)}`}
                      >
                        {provider.name}
                      </Link>
                    </TableCell>
                    <TableCell>
                      <StatusBadge status={provider.current_status} />
                    </TableCell>
                    <TableCell>{percentage(provider.success_rate)}</TableCell>
                    <TableCell>{percentage(provider.error_rate)}</TableCell>
                    <TableCell>{latency(provider.p50_latency_ms)}</TableCell>
                    <TableCell>{latency(provider.p95_latency_ms)}</TableCell>
                    <TableCell>
                      {timestamp(provider.last_successful_request)}
                    </TableCell>
                    <TableCell>{timestamp(provider.last_failure)}</TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </CardContent>
        </Card>

        <div className="mt-6 grid gap-4 xl:grid-cols-3">
          <HealthChart
            title="Availability"
            description="Successful requests over time"
            data={points}
            lines={[
              {
                key: "availability",
                label: "Availability",
                color: "var(--chart-2)",
              },
            ]}
          />
          <HealthChart
            title="Latency"
            description="Provider response latency"
            data={points}
            lines={[
              {
                key: "p50_latency_ms",
                label: "p50",
                color: "var(--chart-3)",
              },
              {
                key: "p95_latency_ms",
                label: "p95",
                color: "var(--chart-4)",
              },
            ]}
          />
          <HealthChart
            title="Requests and errors"
            description="Traffic volume and failed requests"
            data={points}
            lines={[
              {
                key: "requests",
                label: "Requests",
                color: "var(--chart-2)",
              },
              {
                key: "errors",
                label: "Errors",
                color: "var(--destructive)",
              },
            ]}
          />
        </div>
      </DataState>
    </>
  )
}
