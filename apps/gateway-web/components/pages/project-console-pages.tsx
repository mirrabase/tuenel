"use client"

import Link from "next/link"
import * as React from "react"
import { usePathname, useRouter } from "next/navigation"
import {
  ArrowClockwiseIcon,
  ArrowDownIcon,
  CaretLeftIcon,
  CaretRightIcon,
  ArrowUpIcon,
  CopyIcon,
  PencilSimpleIcon,
  PlusIcon,
  PlugsConnectedIcon,
  CheckIcon,
} from "@phosphor-icons/react"
import {
  Bar,
  BarChart,
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
import { RoutingTopology } from "@/components/routing-topology"
import {
  DataState,
  Metric,
  PageHeader,
  StatusBadge,
  TimeRangeSelector,
  type TimeRange,
  useGatewayData,
  useGatewayEndpoint,
} from "@/components/pages/shared"
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog"
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
import {
  Field,
  FieldDescription,
  FieldGroup,
  FieldLabel,
} from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet"
import { Switch } from "@/components/ui/switch"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { gatewayFetch, type Page } from "@/lib/gateway-api"

type Row = Record<string, unknown>
type Resource = Row & {
  id: string
  version?: number
  enabled?: boolean
  created_at?: string
  updated_at?: string
}
const rangeHours: Record<TimeRange, number> = {
  "24h": 24,
  "7d": 168,
  "30d": 720,
}

function projectPath(path: string, tenantId: string, projectId?: string) {
  const query = new URLSearchParams({ tenant_id: tenantId })
  if (projectId) query.set("project_id", projectId)
  return `${path}?${query}`
}

function useRangePath(
  path: string,
  tenantId: string,
  projectId: string | undefined,
  range: TimeRange,
  extra: Record<string, string> = {}
) {
  const [now] = React.useState(Date.now)
  const extraQuery = new URLSearchParams(extra).toString()
  const query = new URLSearchParams(extraQuery)
  query.set("tenant_id", tenantId)
  if (projectId) query.set("project_id", projectId)
  query.set("from", new Date(now - rangeHours[range] * 3_600_000).toISOString())
  query.set("to", new Date(now).toISOString())
  return `${path}?${query}`
}

function canWrite(role: string, gatewayAdmin: boolean) {
  return gatewayAdmin || role === "owner" || role === "admin"
}

function text(value: unknown, fallback = "—") {
  return value === null || value === undefined || value === ""
    ? fallback
    : String(value)
}

function date(value: unknown) {
  return value ? new Date(String(value)).toLocaleString() : "Never"
}

function money(value: unknown) {
  return `$${Number(value ?? 0).toFixed(4)}`
}

function EmptyTableRow({
  columns,
  title,
  description,
}: {
  columns: number
  title: string
  description: string
}) {
  return (
    <TableRow>
      <TableCell colSpan={columns} className="h-36 text-center">
        <p className="font-medium">{title}</p>
        <p className="mt-1 text-sm text-muted-foreground">{description}</p>
      </TableCell>
    </TableRow>
  )
}

function ChartShell({
  title,
  description,
  data,
  dataKey,
  color = "var(--chart-2)",
}: {
  title: string
  description: string
  data: Row[]
  dataKey: string
  color?: string
}) {
  const points = data.length
    ? data
    : Array.from({ length: 8 }, (_, index) => ({
        time: `Bucket ${index + 1}`,
        [dataKey]: 0,
      }))
  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-base">{title}</CardTitle>
        <CardDescription>{description}</CardDescription>
      </CardHeader>
      <CardContent>
        <div className="h-56">
          <ResponsiveContainer width="100%" height="100%">
            <AreaChart data={points}>
              <CartesianGrid strokeDasharray="3 3" vertical={false} />
              <XAxis
                dataKey="time"
                tickFormatter={(value) =>
                  String(value).startsWith("Bucket")
                    ? String(value)
                    : new Date(String(value)).toLocaleDateString(undefined, {
                        month: "short",
                        day: "numeric",
                      })
                }
                fontSize={11}
              />
              <YAxis fontSize={11} width={48} />
              <Tooltip
                labelFormatter={(value) => date(value)}
                contentStyle={{
                  background: "var(--card)",
                  borderColor: "var(--border)",
                  borderRadius: 8,
                }}
              />
              <Area
                type="monotone"
                dataKey={dataKey}
                stroke={color}
                fill={color}
                fillOpacity={0.14}
              />
            </AreaChart>
          </ResponsiveContainer>
        </div>
        {!data.length && (
          <p className="mt-2 text-center text-xs text-muted-foreground">
            No data in this range. Axes remain visible for comparison.
          </p>
        )}
      </CardContent>
    </Card>
  )
}

type BreakdownSection = {
  label: string
  rows?: Row[]
  category: (row: Row) => string
}

function BreakdownTimeSeriesCard({
  label,
  rows,
  events,
  category,
  range = "24h",
}: {
  label: string
  rows?: Row[]
  events: Row[]
  category: (row: Row) => string
  range?: TimeRange
}) {
  const categories = (rows ?? []).map((row) => text(row.name ?? row.id))
  const categoryIndex = new Map(
    categories.map((value, index) => [value, index])
  )

  let numBuckets = 12
  let rangeMs = 24 * 60 * 60 * 1000
  if (range === "7d") {
    numBuckets = 14
    rangeMs = 7 * 24 * 60 * 60 * 1000
  } else if (range === "30d") {
    numBuckets = 15
    rangeMs = 30 * 24 * 60 * 60 * 1000
  } else if (range === "24h") {
    numBuckets = 12
    rangeMs = 24 * 60 * 60 * 1000
  }

  const [now] = React.useState(Date.now)
  const startTime = now - rangeMs
  const bucketMs = rangeMs / numBuckets

  // Create fixed size buckets array for chart
  const chartData = Array.from({ length: numBuckets }, (_, i) => {
    const tStart = startTime + i * bucketMs
    const item: Record<string, unknown> & {
      time: string
      details: Record<
        string,
        { requests: number; tokens: number; cost: number }
      >
    } = {
      time: new Date(tStart).toISOString(),
      details: {},
    }
    categories.forEach((_, idx) => {
      item[`category_${idx}`] = 0
    })
    return item
  })

  events.forEach((event) => {
    const name = category(event)
    if (!name) return
    const time = new Date(String(event.occurred_at)).getTime()
    if (Number.isNaN(time) || time < startTime || time > now) return

    let bucketIdx = Math.floor((time - startTime) / bucketMs)
    if (bucketIdx < 0) bucketIdx = 0
    if (bucketIdx >= numBuckets) bucketIdx = numBuckets - 1

    let idx = categoryIndex.get(name)
    if (idx === undefined) {
      idx = categoryIndex.size
      categoryIndex.set(name, idx)
    }
    const key = `category_${idx}`
    const bucket = chartData[bucketIdx]
    bucket[key] = (Number(bucket[key]) || 0) + 1

    if (!bucket.details[name]) {
      bucket.details[name] = { requests: 0, tokens: 0, cost: 0 }
    }
    bucket.details[name].requests += 1
    bucket.details[name].tokens += Number(
      event.total_tokens ??
        Number(event.prompt_tokens ?? 0) + Number(event.completion_tokens ?? 0)
    )
    bucket.details[name].cost += Number(event.estimated_cost ?? 0)
  })

  const chartCategories = [...categoryIndex.entries()]

  const totalRequests =
    (rows ?? []).reduce((sum, row) => sum + Number(row.requests ?? 0), 0) ||
    events.filter((e) => Boolean(category(e))).length

  const warningCount = events.filter((e) => {
    if (!category(e)) return false
    const st = String(e.status ?? "").toLowerCase()
    return (
      st.includes("warn") ||
      st === "429" ||
      (Number(e.status) >= 400 && Number(e.status) < 500)
    )
  }).length

  const errorCount = events.filter((e) => {
    if (!category(e)) return false
    const st = String(e.status ?? "").toLowerCase()
    return st.includes("fail") || st.includes("err") || Number(e.status) >= 500
  }).length

  const firstTime = new Date(startTime)
  const lastTime = new Date(now)

  return (
    <Card className="flex h-[360px] w-[min(390px,calc(100vw-2rem))] shrink-0 snap-start flex-col overflow-hidden border-border/60 bg-card/95 shadow-sm transition-all hover:border-border">
      <CardHeader className="p-3.5 pb-1">
        <div className="flex items-start justify-between gap-2">
          {/* Left: Label + Big Total Number */}
          <div className="flex flex-col gap-1">
            <span className="text-[10px] font-semibold tracking-wider text-muted-foreground uppercase">
              {label}
            </span>
            <p className="text-2xl leading-none font-bold tracking-tight text-foreground">
              {totalRequests}
            </p>
          </div>

          {/* Right: Warnings & Errors with labels on row 1, values on row 2 */}
          <div className="flex items-start gap-4">
            <div className="flex flex-col items-end gap-1">
              <span className="flex items-center gap-1.5 text-[10px] font-semibold tracking-wider text-muted-foreground uppercase">
                <span className="size-1.5 rounded-full bg-amber-500" />
                <span>WARNINGS</span>
              </span>
              <span className="mt-1 text-sm leading-none font-semibold text-foreground">
                {warningCount}
              </span>
            </div>

            <div className="flex flex-col items-end gap-1">
              <span className="flex items-center gap-1.5 text-[10px] font-semibold tracking-wider text-muted-foreground uppercase">
                <span className="size-1.5 rounded-full bg-rose-500" />
                <span>ERRORS</span>
              </span>
              <span className="mt-1 text-sm leading-none font-semibold text-foreground">
                {errorCount}
              </span>
            </div>
          </div>
        </div>
      </CardHeader>

      <CardContent className="flex min-h-0 flex-1 flex-col gap-2 p-3.5 pt-1">
        <div className="relative flex h-[150px] min-h-0 flex-col justify-between pt-1">
          <div className="h-full min-h-0 w-full">
            <ResponsiveContainer width="100%" height="100%">
              <BarChart
                data={chartData}
                margin={{ top: 4, right: 0, left: 0, bottom: 0 }}
                barCategoryGap="6%"
                maxBarSize={18}
              >
                <YAxis hide />
                <XAxis dataKey="time" hide />
                <Tooltip
                  labelFormatter={(value) => date(value)}
                  cursor={{ fill: "rgba(255, 255, 255, 0.05)" }}
                  content={({ active, label: tooltipLabel, payload }) => {
                    if (!active || !payload?.length) return null
                    const details = payload[0]?.payload?.details ?? {}
                    return (
                      <div className="grid min-w-44 gap-1 rounded-lg border border-border bg-card p-2 text-xs shadow-xl">
                        <p className="font-medium text-foreground">
                          {date(tooltipLabel)}
                        </p>
                        {payload.map((item) => {
                          const detail = details[item.name as string]
                          if (!detail) return null
                          return (
                            <div
                              key={String(item.dataKey)}
                              className="grid gap-0.5"
                            >
                              <span className="font-semibold text-blue-400">
                                {item.name}
                              </span>
                              <span className="text-muted-foreground">
                                {detail.requests.toLocaleString()} requests ·{" "}
                                {detail.tokens.toLocaleString()} tokens ·{" "}
                                {money(detail.cost)}
                              </span>
                            </div>
                          )
                        })}
                      </div>
                    )
                  }}
                />
                {chartCategories.length > 0 ? (
                  chartCategories.map(([name, index]) => (
                    <Bar
                      key={name}
                      dataKey={`category_${index}`}
                      name={name}
                      stackId="requests"
                      fill="#3b82f6"
                      radius={[2, 2, 0, 0]}
                    />
                  ))
                ) : (
                  <Bar
                    dataKey="requests"
                    name="Requests"
                    fill="#3b82f6"
                    radius={[2, 2, 0, 0]}
                  />
                )}
              </BarChart>
            </ResponsiveContainer>
          </div>
          <div className="mt-1 flex items-center justify-between text-[9px] text-muted-foreground/70">
            <span>
              {firstTime.toLocaleDateString(undefined, {
                month: "short",
                day: "numeric",
              })}
              ,{" "}
              {firstTime.toLocaleTimeString([], {
                hour: "numeric",
                minute: "2-digit",
              })}
            </span>
            <span>
              {lastTime.toLocaleDateString(undefined, {
                month: "short",
                day: "numeric",
              })}
              ,{" "}
              {lastTime.toLocaleTimeString([], {
                hour: "numeric",
                minute: "2-digit",
              })}
            </span>
          </div>
        </div>

        <div className="min-h-0 flex-1 [scrollbar-width:thin] space-y-1 overflow-y-auto border-t border-border/40 pt-2">
          {rows?.map((row, index) => (
            <div
              key={text(row.id, String(index))}
              className="flex items-center gap-2 text-[10px]"
            >
              <span className="size-2 shrink-0 rounded-full bg-blue-500" />
              <span className="min-w-0 flex-1 truncate font-medium text-foreground/90">
                {text(row.name ?? row.id)}
              </span>
              <span className="shrink-0 text-muted-foreground">
                {Number(row.requests ?? 0).toLocaleString()} ·{" "}
                {(
                  Number(row.input_tokens ?? 0) + Number(row.output_tokens ?? 0)
                ).toLocaleString()}{" "}
                tok · {money(row.estimated_cost)}
              </span>
            </div>
          ))}
          {!rows?.length && (
            <p className="py-4 text-center text-xs text-muted-foreground/60">
              No {label.toLowerCase()} data recorded
            </p>
          )}
        </div>
      </CardContent>
    </Card>
  )
}

function ConnectModal({
  open,
  onOpenChange,
  endpoint,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  endpoint: string
}) {
  const [copiedTab, setCopiedTab] = React.useState<string | null>(null)

  const baseUrl = endpoint.replace(/\/+$/, "")

  const snippets: Record<
    string,
    { label: string; code: string; lang: string }
  > = {
    curl: {
      label: "cURL",
      lang: "bash",
      code: `curl ${baseUrl}/chat/completions \\
  -H "Content-Type: application/json" \\
  -H "Authorization: Bearer YOUR_VIRTUAL_KEY" \\
  -d '{
    "model": "gateway-default",
    "messages": [
      { "role": "user", "content": "Hello from Tuenel Gateway!" }
    ]
  }'`,
    },
    openai_py: {
      label: "OpenAI Python",
      lang: "python",
      code: `from openai import OpenAI

client = OpenAI(
    base_url="${baseUrl}",
    api_key="YOUR_VIRTUAL_KEY",
)

response = client.chat.completions.create(
    model="gateway-default",
    messages=[{"role": "user", "content": "Hello from Tuenel Gateway!"}],
)

print(response.choices[0].message.content)`,
    },
    openai_js: {
      label: "OpenAI Node.js",
      lang: "typescript",
      code: `import OpenAI from "openai";

const openai = new OpenAI({
  baseURL: "${baseUrl}",
  apiKey: "YOUR_VIRTUAL_KEY",
});

async function main() {
  const response = await openai.chat.completions.create({
  model: "gateway-default",
    messages: [{ role: "user", content: "Hello from Tuenel Gateway!" }],
  });

  console.log(response.choices[0].message.content);
}

main();`,
    },
    langchain_py: {
      label: "LangChain (Py)",
      lang: "python",
      code: `from langchain_openai import ChatOpenAI

llm = ChatOpenAI(
    model="gateway-default",
    base_url="${baseUrl}",
    api_key="YOUR_VIRTUAL_KEY",
)

response = llm.invoke("Hello from Tuenel Gateway!")
print(response.content)`,
    },
    langchain_js: {
      label: "LangChain (JS)",
      lang: "typescript",
      code: `import { ChatOpenAI } from "@langchain/openai";

const model = new ChatOpenAI({
  modelName: "gateway-default",
  configuration: {
    baseURL: "${baseUrl}",
    apiKey: "YOUR_VIRTUAL_KEY",
  },
});

const response = await model.invoke("Hello from Tuenel Gateway!");
console.log(response.content);`,
    },
    vercel_ai: {
      label: "Vercel AI SDK",
      lang: "typescript",
      code: `import { createOpenAI } from "@ai-sdk/openai";
import { generateText } from "ai";

const gateway = createOpenAI({
  baseURL: "${baseUrl}",
  apiKey: "YOUR_VIRTUAL_KEY",
});

const { text } = await generateText({
  model: gateway("gateway-default"),
  prompt: "Hello from Tuenel Gateway!",
});

console.log(text);`,
    },
    llamaindex: {
      label: "LlamaIndex",
      lang: "python",
      code: `from llama_index.llms.openai import OpenAI

llm = OpenAI(
    model="gateway-default",
    api_base="${baseUrl}",
    api_key="YOUR_VIRTUAL_KEY",
)

response = llm.complete("Hello from Tuenel Gateway!")
print(response.text)`,
    },
  }

  function copyCode(key: string, code: string) {
    navigator.clipboard
      .writeText(code)
      .then(() => {
        setCopiedTab(key)
        toast.success("Code snippet copied!")
        setTimeout(() => setCopiedTab(null), 2000)
      })
      .catch(() => toast.error("Failed to copy snippet"))
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl sm:max-w-3xl">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2 text-lg">
            <PlugsConnectedIcon className="size-5 text-primary" />
            Connect to Gateway
          </DialogTitle>
          <DialogDescription>
            Integrate Tuenel OpenAI-compatible gateway into your backend, AI
            frameworks, or CLI tools.
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4 py-1">
          <div className="grid gap-3 rounded-lg border bg-muted/30 p-3 text-xs sm:grid-cols-2">
            <div>
              <span className="text-[10px] font-semibold tracking-wider text-muted-foreground uppercase">
                Base URL
              </span>
              <div className="mt-1 flex items-center justify-between font-mono text-xs text-foreground">
                <span className="truncate">{baseUrl}</span>
                <Button
                  size="icon-xs"
                  variant="ghost"
                  onClick={() =>
                    navigator.clipboard
                      .writeText(baseUrl)
                      .then(() => toast.success("Base URL copied"))
                  }
                >
                  <CopyIcon />
                </Button>
              </div>
            </div>
            <div>
              <span className="text-[10px] font-semibold tracking-wider text-muted-foreground uppercase">
                Authentication
              </span>
              <div className="mt-1 font-mono text-xs text-muted-foreground">
                Pass key as{" "}
                <code className="rounded bg-muted px-1.5 py-0.5 font-semibold text-foreground">
                  Bearer TVK_...
                </code>
              </div>
            </div>
          </div>

          <Tabs defaultValue="curl" className="w-full">
            <TabsList className="flex h-auto w-full flex-wrap justify-start gap-1 bg-muted/50 p-1">
              {Object.entries(snippets).map(([key, item]) => (
                <TabsTrigger
                  key={key}
                  value={key}
                  className="h-7 px-2.5 text-xs"
                >
                  {item.label}
                </TabsTrigger>
              ))}
            </TabsList>

            {Object.entries(snippets).map(([key, item]) => (
              <TabsContent key={key} value={key} className="mt-3">
                <div className="relative overflow-hidden rounded-lg border bg-zinc-950 text-zinc-50 dark:bg-zinc-900">
                  <div className="flex items-center justify-between border-b border-zinc-800 bg-zinc-900/80 px-3 py-1.5 text-[11px] text-zinc-400">
                    <span className="font-mono">{item.lang}</span>
                    <Button
                      size="sm"
                      variant="ghost"
                      className="h-6 px-2 text-[11px] text-zinc-300 hover:bg-zinc-800 hover:text-zinc-100"
                      onClick={() => copyCode(key, item.code)}
                    >
                      {copiedTab === key ? (
                        <>
                          <CheckIcon
                            data-icon="inline-start"
                            className="size-3 text-emerald-400"
                          />
                          Copied!
                        </>
                      ) : (
                        <>
                          <CopyIcon
                            data-icon="inline-start"
                            className="size-3"
                          />
                          Copy Code
                        </>
                      )}
                    </Button>
                  </div>
                  <pre className="max-h-[320px] overflow-x-auto p-4 font-mono text-xs leading-relaxed">
                    <code>{item.code}</code>
                  </pre>
                </div>
              </TabsContent>
            ))}
          </Tabs>
        </div>

        <DialogFooter className="sm:justify-between">
          <p className="text-[11px] text-muted-foreground">
            All endpoints are 100% OpenAI API compatible.
          </p>
          <Button
            variant="outline"
            size="sm"
            onClick={() => onOpenChange(false)}
          >
            Close
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

export function ProjectOverviewPage() {
  const { tenantId, projectId } = useGateway()
  const pathname = usePathname()
  const locale = pathname.split("/")[1]
  const [range, setRange] = React.useState<TimeRange>("24h")
  const [connectOpen, setConnectOpen] = React.useState(false)
  const endpoint = useGatewayEndpoint()
  const usage = useGatewayData<Row>(
    useRangePath("/admin/usage/summary", tenantId, projectId, range)
  )
  const series = useGatewayData<{ data: Row[] }>(
    useRangePath("/admin/usage/series", tenantId, projectId, range, {
      limit: "48",
    })
  )
  const providers = useGatewayData<Page<Resource>>(
    projectPath("/admin/providers", tenantId)
  )
  const keys = useGatewayData<Page<Resource>>(
    projectPath("/admin/virtual-keys", tenantId, projectId)
  )
  const routes = useGatewayData<Page<Resource>>(
    projectPath("/admin/model-routes", tenantId, projectId)
  )
  const requests = useGatewayData<Page<Row>>(
    `${projectPath("/admin/usage/events", tenantId, projectId)}&limit=5`
  )
  const system = useGatewayData<Row>(
    `/admin/system?tenant_id=${encodeURIComponent(tenantId)}`
  )
  const providerRows = providers.data?.data ?? []
  const routeRows = routes.data?.data ?? []
  const requestRows = requests.data?.data ?? []
  const healthRows = Array.isArray(system.data?.providers)
    ? (system.data.providers as Row[])
    : []
  const runtime = (system.data?.runtime ?? {}) as Row
  const activeKeys = (keys.data?.data ?? []).filter((key) => !key.revoked_at)
  const limitedKeys = activeKeys.filter(
    (key) =>
      Number(key.daily_request_limit ?? 0) > 0 ||
      Number(key.daily_token_limit ?? 0) > 0 ||
      Number(key.monthly_budget ?? 0) > 0
  )
  const routedProviderIds = new Set(
    routeRows.map((route) => text(route.provider ?? route.provider_id, ""))
  )
  const routedProviders = providerRows.filter((provider) =>
    routedProviderIds.has(provider.id)
  )
  const loading =
    usage.loading ||
    series.loading ||
    providers.loading ||
    keys.loading ||
    routes.loading ||
    requests.loading ||
    system.loading
  const error =
    usage.error ??
    series.error ??
    providers.error ??
    keys.error ??
    routes.error ??
    requests.error ??
    system.error ??
    undefined
  const summaryMetrics = [
    ["Requests", Number(usage.data?.requests ?? 0).toLocaleString()],
    ["Success", `${Number(usage.data?.success_rate ?? 0).toFixed(1)}%`],
    [
      "p95 latency",
      usage.data?.p95_latency_ms
        ? `${Number(usage.data.p95_latency_ms).toFixed(0)} ms`
        : "—",
    ],
    ["Tokens", Number(usage.data?.total_tokens ?? 0).toLocaleString()],
    ["Est. cost", money(usage.data?.estimated_cost)],
  ]
  const rateLimitStatus = !activeKeys.length
    ? "No active keys"
    : limitedKeys.length
      ? `${limitedKeys.length}/${activeKeys.length} keys limited`
      : "No key limits"
  const trend = series.data?.data ?? []
  const trendPoints = trend.length
    ? trend
    : Array.from({ length: 8 }, (_, index) => ({
        time: `Bucket ${index + 1}`,
        requests: 0,
      }))

  return (
    <>
      <PageHeader
        title={
          <div className="flex items-center gap-3">
            <span>Gateway overview</span>
            <Button size="sm" onClick={() => setConnectOpen(true)}>
              <PlugsConnectedIcon data-icon="inline-start" />
              Connect
            </Button>
          </div>
        }
        action={
          <div className="flex flex-wrap gap-2">
            <Button
              variant="outline"
              onClick={() =>
                navigator.clipboard
                  .writeText(endpoint)
                  .then(() => toast.success("Endpoint copied"))
              }
            >
              <CopyIcon data-icon="inline-start" />
              Copy endpoint
            </Button>
            <Button
              variant="outline"
              render={
                <Link
                  href={`/${locale}/${tenantId}/project/${projectId}/keys`}
                />
              }
            >
              Create API key
            </Button>
            <Button
              render={
                <Link
                  href={`/${locale}/${tenantId}/project/${projectId}/playground`}
                />
              }
            >
              Open Playground
            </Button>
          </div>
        }
      />
      <ConnectModal
        open={connectOpen}
        onOpenChange={setConnectOpen}
        endpoint={endpoint}
      />
      <DataState loading={loading} error={error}>
        <div className="w-full min-w-0 space-y-4">
          <div className="grid min-w-0 gap-4 xl:grid-cols-[minmax(0,2.2fr)_minmax(320px,0.8fr)]">
            <Card
              aria-label="Routing topology"
              className="flex min-w-0 flex-col overflow-hidden"
            >
              <CardContent className="min-w-0 flex-1 p-0">
                <RoutingTopology
                  routes={routeRows}
                  providers={providerRows}
                  modelsHref={`/${locale}/${tenantId}/project/${projectId}/models`}
                  className="rounded-none border-none"
                />
              </CardContent>
            </Card>
            <Card className="flex h-full min-w-0 flex-col">
              <CardHeader>
                <div className="flex items-start justify-between gap-3">
                  <div>
                    <CardTitle>Project summary</CardTitle>
                    <CardDescription>Selected range: {range}</CardDescription>
                  </div>
                  <TimeRangeSelector value={range} onChange={setRange} />
                </div>
              </CardHeader>
              <CardContent className="flex flex-1 flex-col justify-between space-y-4">
                <div className="grid grid-cols-2 gap-px overflow-hidden rounded-md border bg-border">
                  {summaryMetrics.map(([label, detail], index) => (
                    <div
                      key={label}
                      className={`bg-card p-3 ${index === summaryMetrics.length - 1 ? "col-span-2" : ""}`}
                    >
                      <p className="text-[10px] text-muted-foreground uppercase">
                        {label}
                      </p>
                      <p className="mt-1 font-mono text-lg font-semibold">
                        {detail}
                      </p>
                    </div>
                  ))}
                </div>
                <div className="divide-y rounded-md border">
                  {[
                    ["Gateway health", text(runtime.state, "Unknown")],
                    ["Active API keys", String(activeKeys.length)],
                    ["Rate limits", rateLimitStatus],
                    ["Last request", date(requestRows[0]?.occurred_at)],
                  ].map(([label, detail]) => (
                    <div
                      key={label}
                      className="flex items-center justify-between gap-4 px-3 py-2"
                    >
                      <span className="text-muted-foreground">{label}</span>
                      <span className="truncate text-right font-medium">
                        {detail}
                      </span>
                    </div>
                  ))}
                </div>
              </CardContent>
            </Card>
          </div>
          <div className="grid min-w-0 gap-4 xl:grid-cols-[minmax(0,2.2fr)_minmax(320px,0.8fr)]">
            <Card className="min-w-0">
              <CardHeader>
                <CardTitle>Recent requests</CardTitle>
                <CardDescription>
                  Latest inference activity for this project.
                </CardDescription>
              </CardHeader>
              <CardContent className="min-w-0 overflow-x-auto">
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>Time</TableHead>
                      <TableHead>Status</TableHead>
                      <TableHead>Alias</TableHead>
                      <TableHead>Provider</TableHead>
                      <TableHead className="text-right">Latency</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {!requestRows.length && (
                      <EmptyTableRow
                        columns={5}
                        title="No recent requests"
                        description="Requests appear after this project sends inference traffic."
                      />
                    )}
                    {requestRows.map((request, index) => (
                      <TableRow key={text(request.request_id, String(index))}>
                        <TableCell>{date(request.occurred_at)}</TableCell>
                        <TableCell>
                          <StatusBadge status={text(request.status)} />
                        </TableCell>
                        <TableCell className="font-mono">
                          {text(request.requested_model)}
                        </TableCell>
                        <TableCell>{text(request.provider)}</TableCell>
                        <TableCell className="text-right font-mono">
                          {request.latency_ms
                            ? `${Number(request.latency_ms).toFixed(0)} ms`
                            : "—"}
                        </TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              </CardContent>
            </Card>
            <div className="grid min-w-0 gap-4">
              <Card className="min-w-0">
                <CardHeader>
                  <CardTitle>Provider health</CardTitle>
                  <CardDescription>
                    Providers referenced by project routes.
                  </CardDescription>
                </CardHeader>
                <CardContent className="max-h-52 space-y-2 overflow-y-auto">
                  {!routedProviders.length && (
                    <p className="py-4 text-center text-xs text-muted-foreground">
                      No routed providers to monitor.
                    </p>
                  )}
                  {routedProviders.map((provider) => {
                    const health = healthRows.find(
                      (row) => text(row.provider_id) === provider.id
                    )
                    return (
                      <div
                        key={provider.id}
                        className="flex items-center justify-between gap-3 rounded-md border px-3 py-2"
                      >
                        <div className="min-w-0">
                          <p className="truncate font-medium">
                            {text(provider.name, provider.id)}
                          </p>
                          <p className="truncate text-[10px] text-muted-foreground">
                            Checked {date(health?.updated_at)}
                          </p>
                        </div>
                        <StatusBadge
                          status={text(
                            health?.status,
                            provider.enabled === false
                              ? "Disabled"
                              : "Configured"
                          )}
                        />
                      </div>
                    )
                  })}
                </CardContent>
              </Card>
              <Card className="min-w-0">
                <CardHeader>
                  <CardTitle>Usage trend</CardTitle>
                  <CardDescription>Requests across {range}.</CardDescription>
                </CardHeader>
                <CardContent>
                  <div className="h-36">
                    <ResponsiveContainer width="100%" height="100%">
                      <AreaChart data={trendPoints}>
                        <CartesianGrid strokeDasharray="3 3" vertical={false} />
                        <XAxis dataKey="time" hide />
                        <YAxis hide />
                        <Tooltip
                          labelFormatter={(value) => date(value)}
                          contentStyle={{
                            background: "var(--card)",
                            borderColor: "var(--border)",
                            borderRadius: 8,
                          }}
                        />
                        <Area
                          type="monotone"
                          dataKey="requests"
                          stroke="var(--chart-2)"
                          fill="var(--chart-2)"
                          fillOpacity={0.16}
                        />
                      </AreaChart>
                    </ResponsiveContainer>
                  </div>
                  {!trend.length && (
                    <p className="mt-1 text-center text-[10px] text-muted-foreground">
                      No usage recorded in this range.
                    </p>
                  )}
                </CardContent>
              </Card>
            </div>
          </div>
        </div>
      </DataState>
    </>
  )
}

type VirtualKey = Resource & {
  display_name?: string
  key_prefix?: string
  expires_at?: string
  revoked_at?: string
  last_used_at?: string
  daily_token_limit?: number
  daily_request_limit?: number
  monthly_budget?: number
  allowed_models?: string[]
}

export function ApiKeysPage() {
  const session = useGateway()
  const state = useGatewayData<Page<VirtualKey>>(
    projectPath("/admin/virtual-keys", session.tenantId, session.projectId)
  )
  const models = useGatewayData<{ data: { id: string }[] }>(
    "/v1/models",
    session.projectId
  )
  const [creating, setCreating] = React.useState(false)
  const [issued, setIssued] = React.useState("")
  const [revoking, setRevoking] = React.useState<VirtualKey>()
  const [pending, setPending] = React.useState(false)
  const writable = canWrite(session.tenantRole, session.gatewayAdmin)

  async function create(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const form = new FormData(event.currentTarget)
    setPending(true)
    try {
      const result = await gatewayFetch<{ key: string }>(
        "/admin/virtual-keys",
        session.tenantId,
        {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({
            display_name: form.get("name"),
            project_id: session.projectId,
            scopes: ["inference"],
            expires_at: form.get("expires_at")
              ? new Date(String(form.get("expires_at"))).toISOString()
              : null,
            allowed_models: form.getAll("allowed_models"),
            daily_request_limit: Number(form.get("request_limit")) || null,
            daily_token_limit: Number(form.get("token_limit")),
            monthly_budget: Number(form.get("budget")) || null,
          }),
        }
      )
      setCreating(false)
      setIssued(result.key)
      state.reload()
    } catch (error) {
      toast.error(
        error instanceof Error ? error.message : "Key creation failed"
      )
    } finally {
      setPending(false)
    }
  }

  async function revoke() {
    if (!revoking) return
    setPending(true)
    try {
      await gatewayFetch(
        `/admin/virtual-keys/${revoking.id}?project_id=${encodeURIComponent(session.projectId ?? "")}`,
        session.tenantId,
        { method: "DELETE" }
      )
      setRevoking(undefined)
      state.reload()
      toast.success("API key revoked")
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Revoke failed")
    } finally {
      setPending(false)
    }
  }

  return (
    <>
      <PageHeader
        title="API Keys"
        action={
          <Button disabled={!writable} onClick={() => setCreating(true)}>
            <PlusIcon data-icon="inline-start" />
            Create API key
          </Button>
        }
      />
      <Card>
        <CardContent>
          <DataState
            loading={state.loading}
            error={state.error}
            onRetry={state.reload}
          >
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Name</TableHead>
                  <TableHead>Prefix</TableHead>
                  <TableHead>Status</TableHead>
                  <TableHead>Created</TableHead>
                  <TableHead>Last used</TableHead>
                  <TableHead>Limits</TableHead>
                  <TableHead />
                </TableRow>
              </TableHeader>
              <TableBody>
                {!state.data?.data.length && (
                  <EmptyTableRow
                    columns={7}
                    title="No API keys"
                    description="Create a project key to call this gateway from an application."
                  />
                )}
                {state.data?.data.map((key) => (
                  <TableRow key={key.id}>
                    <TableCell className="font-medium">
                      {text(key.display_name, "Unnamed key")}
                    </TableCell>
                    <TableCell className="font-mono">
                      {text(key.key_prefix)}
                    </TableCell>
                    <TableCell>
                      <StatusBadge
                        status={
                          key.revoked_at
                            ? "Revoked"
                            : key.expires_at &&
                                new Date(key.expires_at) < new Date()
                              ? "Expired"
                              : "Active"
                        }
                      />
                    </TableCell>
                    <TableCell>{date(key.created_at)}</TableCell>
                    <TableCell>{date(key.last_used_at)}</TableCell>
                    <TableCell className="text-xs">
                      {Number(key.daily_request_limit ?? 0) > 0 && (
                        <div>
                          {Number(key.daily_request_limit).toLocaleString()}{" "}
                          req/day
                        </div>
                      )}
                      <div>
                        {Number(key.daily_token_limit ?? 0).toLocaleString()}{" "}
                        tokens/day
                      </div>
                      {Number(key.monthly_budget ?? 0) > 0 && (
                        <div>{money(key.monthly_budget)} / month</div>
                      )}
                    </TableCell>
                    <TableCell className="text-right">
                      <Button
                        size="sm"
                        variant="outline"
                        disabled={!writable || Boolean(key.revoked_at)}
                        onClick={() => setRevoking(key)}
                      >
                        Revoke
                      </Button>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </DataState>
        </CardContent>
      </Card>
      <Dialog
        open={creating}
        onOpenChange={(open) => !pending && setCreating(open)}
      >
        <DialogContent className="sm:max-w-xl">
          <DialogHeader>
            <DialogTitle>Create API key</DialogTitle>
            <DialogDescription>
              The plaintext credential is shown once after creation.
            </DialogDescription>
          </DialogHeader>
          <form onSubmit={create}>
            <FieldGroup>
              <Field>
                <FieldLabel htmlFor="key-name">Name</FieldLabel>
                <Input id="key-name" name="name" required maxLength={100} />
              </Field>
              <Field>
                <FieldLabel htmlFor="key-expiration">Expiration</FieldLabel>
                <Input
                  id="key-expiration"
                  name="expires_at"
                  type="datetime-local"
                />
              </Field>
              <Field>
                <FieldLabel>Allowed models</FieldLabel>
                <div className="grid gap-2 sm:grid-cols-2">
                  {models.data?.data.map((model) => (
                    <label
                      key={model.id}
                      className="flex gap-2 rounded-md border p-2 text-sm"
                    >
                      <input
                        type="checkbox"
                        name="allowed_models"
                        value={model.id}
                        defaultChecked
                      />
                      <span className="font-mono">{model.id}</span>
                    </label>
                  ))}
                  {!models.data?.data.length && (
                    <p className="text-sm text-muted-foreground">
                      No project aliases are available.
                    </p>
                  )}
                </div>
              </Field>
              <div className="grid gap-4 sm:grid-cols-3">
                <Field>
                  <FieldLabel>Requests / day</FieldLabel>
                  <Input name="request_limit" type="number" min={1} />
                </Field>
                <Field>
                  <FieldLabel>Tokens / day</FieldLabel>
                  <Input name="token_limit" type="number" min={1} required />
                </Field>
                <Field>
                  <FieldLabel>Budget / month</FieldLabel>
                  <Input name="budget" type="number" min={0} step="0.01" />
                </Field>
              </div>
              <DialogFooter>
                <Button type="submit" disabled={pending}>
                  {pending ? "Creating…" : "Create API key"}
                </Button>
              </DialogFooter>
            </FieldGroup>
          </form>
        </DialogContent>
      </Dialog>
      <Dialog
        open={Boolean(issued)}
        onOpenChange={(open) => {
          if (!open) setIssued("")
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Copy your API key</DialogTitle>
            <DialogDescription>
              This secret cannot be shown again. Store it in a secrets manager.
            </DialogDescription>
          </DialogHeader>
          <div className="rounded-md border bg-muted p-3 font-mono text-sm break-all">
            {issued}
          </div>
          <DialogFooter>
            <Button
              onClick={() =>
                navigator.clipboard
                  .writeText(issued)
                  .then(() => toast.success("API key copied"))
              }
            >
              <CopyIcon data-icon="inline-start" />
              Copy API key
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
      <AlertDialog
        open={Boolean(revoking)}
        onOpenChange={(open) => !open && setRevoking(undefined)}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Revoke this API key?</AlertDialogTitle>
            <AlertDialogDescription>
              Applications using{" "}
              {text(revoking?.display_name, revoking?.key_prefix)}
              will immediately lose access.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction variant="destructive" onClick={revoke}>
              Revoke API key
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  )
}

function RouteEditor({
  route,
  open,
  onOpenChange,
  onSaved,
}: {
  route?: Resource
  open: boolean
  onOpenChange: (open: boolean) => void
  onSaved: () => void
}) {
  const session = useGateway()
  const providers = useGatewayData<Page<Resource>>(
    projectPath("/admin/providers", session.tenantId)
  )
  const existingRoutes = useGatewayData<Page<Resource>>(
    projectPath("/admin/model-routes", session.tenantId, session.projectId)
  )
  const aliases = [
    ...new Set(
      (existingRoutes.data?.data ?? []).map((item) =>
        text(item.requested_model, "")
      )
    ),
  ].filter(Boolean).sort()
  const [aliasMode, setAliasMode] = React.useState(
    route ? text(route.requested_model, "") : "__new__"
  )
  const [providerId, setProviderId] = React.useState<string>()
  const selectedProvider =
    providerId ?? text(route?.provider, providers.data?.data[0]?.id ?? "")
  const [modelOptions, setModelOptions] = React.useState<{
    provider: string
    data: string[]
  }>({ provider: "", data: [] })
  const models =
    modelOptions.provider === selectedProvider ? modelOptions.data : []
  const [pending, setPending] = React.useState(false)
  React.useEffect(() => {
    if (!selectedProvider) return
    let current = true
    gatewayFetch<{ data: { id: string }[] }>(
      `/admin/providers/${encodeURIComponent(selectedProvider)}/models`,
      session.tenantId
    )
      .then((result) => {
        if (current)
          setModelOptions({
            provider: selectedProvider,
            data: result.data.map((model) => model.id),
          })
      })
      .catch(() => {})
    return () => {
      current = false
    }
  }, [selectedProvider, session.tenantId])
  async function save(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const form = new FormData(event.currentTarget)
    setPending(true)
    try {
      await gatewayFetch(
        route ? `/admin/model-routes/${route.id}` : "/admin/model-routes",
        session.tenantId,
        {
          method: route ? "PATCH" : "POST",
          headers: {
            "content-type": "application/json",
            ...(route ? { "if-match": `"${route.version ?? 1}"` } : {}),
          },
          body: JSON.stringify({
            tenant_id: route ? undefined : session.tenantId,
            project_id: session.projectId,
            requested_model: form.get("alias"),
            provider: form.get("provider"),
            upstream_model: form.get("upstream_model"),
            priority: Number(form.get("priority")),
            enabled: form.get("enabled") === "on",
          }),
        }
      )
      onOpenChange(false)
      onSaved()
      toast.success(route ? "Routing target updated" : "Routing target created")
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Save failed")
    } finally {
      setPending(false)
    }
  }
  return (
    <Dialog
      open={open}
      onOpenChange={(value) => !pending && onOpenChange(value)}
    >
      <DialogContent>
        <DialogHeader>
          <DialogTitle>
            {route ? "Edit routing target" : "Create routing rule"}
          </DialogTitle>
          <DialogDescription>
            Applications call the alias; Tunel selects targets by ascending
            priority.
          </DialogDescription>
        </DialogHeader>
        <form onSubmit={save}>
          <FieldGroup>
            <Field>
              <FieldLabel>Alias</FieldLabel>
              {route ? (
                <Input
                  name="alias"
                  defaultValue={text(route.requested_model, "")}
                  readOnly
                  required
                />
              ) : (
                <>
                  <Select
                    value={aliasMode}
                    onValueChange={(value) => value && setAliasMode(value)}
                  >
                    <SelectTrigger className="w-full">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="__new__">Create new alias</SelectItem>
                      {aliases.map((alias) => (
                        <SelectItem key={alias} value={alias}>
                          {alias}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                  {aliasMode === "__new__" ? (
                    <Input
                      name="alias"
                      placeholder="e.g. gateway-default"
                      required
                    />
                  ) : (
                    <input type="hidden" name="alias" value={aliasMode} />
                  )}
                </>
              )}
            </Field>
            <Field>
              <FieldLabel>Provider</FieldLabel>
              <Select
                name="provider"
                value={selectedProvider}
                onValueChange={(value) => value && setProviderId(value)}
              >
                <SelectTrigger className="w-full">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {providers.data?.data.map((provider) => (
                    <SelectItem key={provider.id} value={provider.id}>
                      {text(provider.name, provider.id)}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </Field>
            <Field>
              <FieldLabel>Upstream model</FieldLabel>
              <Input
                name="upstream_model"
                list="provider-models"
                defaultValue={text(route?.upstream_model, "")}
                placeholder="Search or enter a custom model ID"
                required
              />
              <datalist id="provider-models">
                {models.map((model) => (
                  <option key={model} value={model} />
                ))}
              </datalist>
              <FieldDescription>
                Select a model exposed by this provider, or enter a custom ID.
              </FieldDescription>
            </Field>
            <Field>
              <FieldLabel>Priority</FieldLabel>
              <Input
                name="priority"
                type="number"
                min={1}
                value={
                  route
                    ? Number(route.priority ?? 1)
                    : aliasMode === "__new__"
                      ? 1
                      : (existingRoutes.data?.data ?? []).filter(
                            (item) =>
                              text(item.requested_model, "") === aliasMode
                          ).length + 1
                }
                readOnly
                required
              />
              <FieldDescription>
                Priority is normalized automatically. Reorder targets from the
                Routing page.
              </FieldDescription>
            </Field>
            <Field orientation="horizontal">
              <Switch
                name="enabled"
                defaultChecked={route?.enabled !== false}
              />
              <FieldLabel>Enabled</FieldLabel>
            </Field>
            <DialogFooter>
              <Button type="submit" disabled={pending}>
                {pending ? "Saving…" : "Save routing target"}
              </Button>
            </DialogFooter>
          </FieldGroup>
        </form>
      </DialogContent>
    </Dialog>
  )
}

export function ModelsPage() {
  const session = useGateway()
  const routes = useGatewayData<Page<Resource>>(
    projectPath("/admin/model-routes", session.tenantId, session.projectId)
  )
  const [editor, setEditor] = React.useState<Resource | "new">()
  const grouped = Object.entries(
    (routes.data?.data ?? []).reduce<Record<string, Resource[]>>(
      (all, route) => {
        const alias = text(route.requested_model)
        ;(all[alias] ??= []).push(route)
        return all
      },
      {}
    )
  )
  return (
    <>
      <PageHeader
        title="Models"
        action={
          <Button
            disabled={!canWrite(session.tenantRole, session.gatewayAdmin)}
            onClick={() => setEditor("new")}
          >
            <PlusIcon data-icon="inline-start" />
            Create model alias
          </Button>
        }
      />
      <Card>
        <CardContent>
          <DataState
            loading={routes.loading}
            error={routes.error}
            onRetry={routes.reload}
          >
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Alias</TableHead>
                  <TableHead>Provider</TableHead>
                  <TableHead>Upstream model</TableHead>
                  <TableHead>Routing policy</TableHead>
                  <TableHead>Status</TableHead>
                  <TableHead>Last updated</TableHead>
                  <TableHead />
                </TableRow>
              </TableHeader>
              <TableBody>
                {!grouped.length && (
                  <EmptyTableRow
                    columns={7}
                    title="No model aliases"
                    description="Create an alias and primary provider target to expose a model."
                  />
                )}
                {grouped.map(([alias, targets]) => {
                  const ordered = [...targets].sort(
                    (a, b) => Number(a.priority) - Number(b.priority)
                  )
                  const primary = ordered[0]
                  return (
                    <TableRow key={alias}>
                      <TableCell className="font-mono font-medium">
                        {alias}
                      </TableCell>
                      <TableCell>
                        <div className="space-y-1">
                          {ordered.map((target, index) => (
                            <p key={String(target.id)}>
                              {index === 0 ? "Primary: " : `Fallback ${index}: `}
                              {text(target.provider)}
                            </p>
                          ))}
                        </div>
                      </TableCell>
                      <TableCell className="font-mono">
                        <div className="space-y-1">
                          {ordered.map((target) => (
                            <p key={String(target.id)}>
                              {text(target.upstream_model)}
                            </p>
                          ))}
                        </div>
                      </TableCell>
                      <TableCell>
                        {ordered.length > 1
                          ? `${ordered.length - 1} fallbacks`
                          : "Primary only"}
                      </TableCell>
                      <TableCell>
                        <StatusBadge
                          status={
                            primary.enabled === false ? "Disabled" : "Active"
                          }
                        />
                      </TableCell>
                      <TableCell>{date(primary.updated_at)}</TableCell>
                      <TableCell className="text-right">
                        <Button
                          size="sm"
                          variant="outline"
                          onClick={() => setEditor(primary)}
                        >
                          <PencilSimpleIcon data-icon="inline-start" />
                          Edit alias
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
      <RouteEditor
        key={editor === "new" ? "new" : (editor?.id ?? "closed")}
        route={editor === "new" ? undefined : editor}
        open={Boolean(editor)}
        onOpenChange={(open) => !open && setEditor(undefined)}
        onSaved={routes.reload}
      />
    </>
  )
}

export function RoutingPage() {
  const session = useGateway()
  const routes = useGatewayData<Page<Resource>>(
    projectPath("/admin/model-routes", session.tenantId, session.projectId)
  )
  const [editor, setEditor] = React.useState<Resource | "new">()
  const [advanced, setAdvanced] = React.useState<Resource>()
  const writable = canWrite(session.tenantRole, session.gatewayAdmin)
  async function move(route: Resource, delta: number) {
    try {
      await gatewayFetch(`/admin/model-routes/${route.id}`, session.tenantId, {
        method: "PATCH",
        headers: {
          "content-type": "application/json",
          "if-match": `"${route.version ?? 1}"`,
        },
        body: JSON.stringify({
          priority:
            delta > 0
              ? Number(route.priority) + 2
              : Math.max(1, Number(route.priority) - 1),
        }),
      })
      routes.reload()
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Reorder failed")
    }
  }
  return (
    <>
      <PageHeader
        title="Routing"
        action={
          <Button disabled={!writable} onClick={() => setEditor("new")}>
            <PlusIcon data-icon="inline-start" />
            Create routing rule
          </Button>
        }
      />
      <DataState
        loading={routes.loading}
        error={routes.error}
        empty={!routes.data?.data.length}
        onRetry={routes.reload}
        emptyTitle="No routing rules"
        emptyDescription="Create a rule to connect a public alias to an organization provider."
      >
        <div className="space-y-3">
          {[...(routes.data?.data ?? [])]
            .sort(
              (a, b) =>
                text(a.requested_model).localeCompare(
                  text(b.requested_model)
                ) || Number(a.priority) - Number(b.priority)
            )
            .map((route) => (
              <Card key={route.id}>
                <CardContent className="flex flex-col gap-4 py-4 lg:flex-row lg:items-center">
                  <div className="min-w-44">
                    <p className="font-mono font-semibold">
                      {text(route.requested_model)}
                    </p>
                    <StatusBadge
                      status={route.enabled === false ? "Disabled" : "Active"}
                    />
                  </div>
                  <div className="grid flex-1 gap-3 text-sm sm:grid-cols-4">
                    <div>
                      <p className="text-xs text-muted-foreground">Target</p>
                      <p>
                        {text(route.provider)} /{" "}
                        <span className="font-mono">
                          {text(route.upstream_model)}
                        </span>
                      </p>
                    </div>
                    <div>
                      <p className="text-xs text-muted-foreground">Priority</p>
                      <p>{text(route.priority)}</p>
                    </div>
                    <div>
                      <p className="text-xs text-muted-foreground">Timeout</p>
                      <p>{text(route.timeout_ms, "Gateway default")}</p>
                    </div>
                    <div>
                      <p className="text-xs text-muted-foreground">
                        Retry policy
                      </p>
                      <p>
                        {text(route.retry_policy, "Transient failures only")}
                      </p>
                    </div>
                  </div>
                  <div className="flex gap-1">
                    <Button
                      size="icon-sm"
                      variant="outline"
                      aria-label="Move target up"
                      disabled={!writable}
                      onClick={() => move(route, -1)}
                    >
                      <ArrowUpIcon />
                    </Button>
                    <Button
                      size="icon-sm"
                      variant="outline"
                      aria-label="Move target down"
                      disabled={!writable}
                      onClick={() => move(route, 1)}
                    >
                      <ArrowDownIcon />
                    </Button>
                    <Button
                      size="sm"
                      variant="outline"
                      onClick={() => setEditor(route)}
                    >
                      Edit
                    </Button>
                    <Button
                      size="sm"
                      variant="ghost"
                      onClick={() => setAdvanced(route)}
                    >
                      Advanced
                    </Button>
                  </div>
                </CardContent>
              </Card>
            ))}
        </div>
      </DataState>
      <RouteEditor
        route={editor === "new" ? undefined : editor}
        open={Boolean(editor)}
        onOpenChange={(open) => !open && setEditor(undefined)}
        onSaved={routes.reload}
      />
      <Sheet
        open={Boolean(advanced)}
        onOpenChange={(open) => !open && setAdvanced(undefined)}
      >
        <SheetContent className="overflow-y-auto">
          <SheetHeader>
            <SheetTitle>Advanced routing details</SheetTitle>
            <SheetDescription>
              Sanitized control-plane representation.
            </SheetDescription>
          </SheetHeader>
          <pre className="m-4 overflow-auto rounded-md bg-muted p-3 text-xs whitespace-pre-wrap">
            {JSON.stringify(advanced, null, 2)}
          </pre>
        </SheetContent>
      </Sheet>
    </>
  )
}

function BreakdownScroller({ children }: { children: React.ReactNode }) {
  const ref = React.useRef<HTMLDivElement>(null)
  const [hasOverflow, setHasOverflow] = React.useState(false)

  React.useEffect(() => {
    const element = ref.current
    if (!element) return
    const update = () =>
      setHasOverflow(element.scrollWidth > element.clientWidth + 1)
    update()
    const observer = new ResizeObserver(update)
    observer.observe(element)
    return () => observer.disconnect()
  }, [])

  return (
    <div className="group relative max-w-full min-w-0">
      <div
        ref={ref}
        className="[scrollbar-width:none] overflow-x-auto overscroll-x-contain pb-2 [-ms-overflow-style:none] [&::-webkit-scrollbar]:hidden"
      >
        <div className="flex w-max min-w-full snap-x gap-3">{children}</div>
      </div>
      {hasOverflow && (
        <div className="pointer-events-none absolute inset-y-0 right-0 left-0 flex items-center justify-between px-1">
          <Button
            aria-label="Scroll breakdown charts left"
            className="pointer-events-auto size-8 rounded-full border border-zinc-700 bg-zinc-900/90 text-zinc-100 opacity-70 shadow-md backdrop-blur transition-opacity hover:opacity-100"
            variant="ghost"
            size="icon"
            onClick={() =>
              ref.current?.scrollBy({ left: -320, behavior: "smooth" })
            }
          >
            <CaretLeftIcon className="size-4" />
          </Button>
          <Button
            aria-label="Scroll breakdown charts right"
            className="pointer-events-auto size-8 rounded-full border border-zinc-700 bg-zinc-900/90 text-zinc-100 opacity-70 shadow-md backdrop-blur transition-opacity hover:opacity-100"
            variant="ghost"
            size="icon"
            onClick={() =>
              ref.current?.scrollBy({ left: 320, behavior: "smooth" })
            }
          >
            <CaretRightIcon className="size-4" />
          </Button>
        </div>
      )}
    </div>
  )
}

export function UsageCostPage() {
  const session = useGateway()
  const [range, setRange] = React.useState<TimeRange>("7d")
  const summary = useGatewayData<Row>(
    useRangePath(
      "/admin/usage/summary",
      session.tenantId,
      session.projectId,
      range
    )
  )
  const series = useGatewayData<{ data: Row[] }>(
    useRangePath(
      "/admin/usage/series",
      session.tenantId,
      session.projectId,
      range,
      { limit: "100" }
    )
  )
  const breakdowns = useGatewayData<Record<string, Row[]>>(
    useRangePath(
      "/admin/usage/breakdowns",
      session.tenantId,
      session.projectId,
      range
    )
  )
  const events = useGatewayData<Page<Row>>(
    useRangePath(
      "/admin/usage/events",
      session.tenantId,
      session.projectId,
      range,
      { limit: "100" }
    )
  )
  const values = summary.data ?? {}
  const points = series.data?.data ?? []
  const breakdownSections: BreakdownSection[] = [
    {
      label: "Provider",
      rows: breakdowns.data?.providers,
      category: (row) => text(row.provider),
    },
    {
      label: "Model alias",
      rows: breakdowns.data?.models,
      category: (row) => text(row.requested_model),
    },
    {
      label: "Upstream model",
      rows: breakdowns.data?.upstream_models,
      category: (row) => text(row.upstream_model),
    },
    {
      label: "API key",
      rows: breakdowns.data?.api_keys,
      category: (row) => text(row.api_key_name, "Session / JWT"),
    },
    {
      label: "Status",
      rows: breakdowns.data?.statuses,
      category: (row) => text(row.status),
    },
  ]
  function exportCsv() {
    const escape = (value: unknown) =>
      `"${String(value ?? "").replaceAll('"', '""')}"`
    const rows = [
      [
        "timestamp",
        "status",
        "alias",
        "provider",
        "upstream_model",
        "input_tokens",
        "output_tokens",
        "cost",
        "request_id",
      ],
      ...(events.data?.data ?? []).map((row) => [
        row.occurred_at,
        row.status,
        row.requested_model,
        row.provider,
        row.upstream_model,
        row.prompt_tokens,
        row.completion_tokens,
        row.estimated_cost,
        row.request_id,
      ]),
    ]
    const url = URL.createObjectURL(
      new Blob([rows.map((row) => row.map(escape).join(",")).join("\n")], {
        type: "text/csv;charset=utf-8",
      })
    )
    const link = document.createElement("a")
    link.href = url
    link.download = `tuenel-usage-${range}.csv`
    link.click()
    URL.revokeObjectURL(url)
  }
  return (
    <div className="max-w-full min-w-0 overflow-x-hidden">
      <PageHeader
        title="Usage & Cost"
        action={
          <div className="flex gap-2">
            <TimeRangeSelector value={range} onChange={setRange} />
            <Button variant="outline" onClick={exportCsv}>
              Export CSV
            </Button>
          </div>
        }
      />
      <DataState
        loading={summary.loading}
        error={summary.error}
        onRetry={summary.reload}
      >
        <div className="grid min-w-0 gap-4 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-6">
          <Metric
            label="Requests"
            value={Number(values.requests ?? 0).toLocaleString()}
            detail={range}
          />
          <Metric
            label="Input tokens"
            value={Number(values.input_tokens ?? 0).toLocaleString()}
            detail="Prompt usage"
          />
          <Metric
            label="Output tokens"
            value={Number(values.output_tokens ?? 0).toLocaleString()}
            detail="Completion usage"
          />
          <Metric
            label="Total cost"
            value={money(values.estimated_cost)}
            detail={
              Number(values.unpriced_requests ?? 0) > 0
                ? `${Number(values.unpriced_requests).toLocaleString()} requests unpriced`
                : "Estimated"
            }
          />
          <Metric
            label="Average cost/request"
            value={money(
              values.average_cost_per_request ??
                (Number(values.requests)
                  ? Number(values.estimated_cost) / Number(values.requests)
                  : 0)
            )}
            detail={
              Number(values.unpriced_requests ?? 0) > 0
                ? "Priced requests only"
                : "Estimated"
            }
          />
          <Metric
            label="p95 latency"
            value={
              values.p95_latency_ms
                ? `${Number(values.p95_latency_ms).toFixed(0)} ms`
                : "—"
            }
            detail="Completed requests"
          />
        </div>
      </DataState>
      <div className="mt-6 grid min-w-0 gap-6 lg:grid-cols-3">
        <ChartShell
          title="Requests"
          description="Inference requests over time"
          data={points}
          dataKey="requests"
        />
        <ChartShell
          title="Tokens"
          description="Input and output tokens over time"
          data={points}
          dataKey="tokens"
          color="var(--chart-3)"
        />
        <ChartShell
          title="Cost"
          description="Estimated cost over time"
          data={points}
          dataKey="cost"
          color="var(--chart-4)"
        />
      </div>
      <div className="mt-6 max-w-full min-w-0">
        <div className="mb-3 flex flex-wrap items-center justify-between gap-2 px-1">
          <div className="flex items-center gap-3 text-sm font-medium">
            <span className="font-semibold text-muted-foreground/60">::</span>
            <span className="text-sm font-bold tracking-tight text-foreground">
              {Number(values.requests ?? 0).toLocaleString()} Total Requests
            </span>
            <span className="text-sm font-semibold text-blue-500">
              {Number(values.success_rate ?? 100).toFixed(1)}% Success Rate
            </span>
          </div>
        </div>
        <BreakdownScroller>
          {breakdownSections.map(({ label, rows, category }) => (
            <BreakdownTimeSeriesCard
              key={label}
              label={label}
              rows={rows}
              events={events.data?.data ?? []}
              category={category}
              range={range}
            />
          ))}
        </BreakdownScroller>
      </div>
    </div>
  )
}

export function RequestsPage() {
  const session = useGateway()
  const [range, setRange] = React.useState<TimeRange>("24h")
  const [status, setStatus] = React.useState("all")
  const [provider, setProvider] = React.useState("")
  const [model, setModel] = React.useState("")
  const [apiKey, setApiKey] = React.useState("")
  const [requestId, setRequestId] = React.useState("")
  const [selected, setSelected] = React.useState<Row>()
  const state = useGatewayData<Page<Row>>(
    useRangePath(
      "/admin/usage/events",
      session.tenantId,
      session.projectId,
      range,
      { limit: "100" }
    )
  )
  const rows = (state.data?.data ?? []).filter((row) => {
    if (status !== "all" && text(row.status) !== status) return false
    if (
      provider &&
      !text(row.provider, "").toLowerCase().includes(provider.toLowerCase())
    )
      return false
    if (
      model &&
      !text(row.requested_model, "").toLowerCase().includes(model.toLowerCase())
    )
      return false
    if (
      apiKey &&
      !text(row.principal_id, "").toLowerCase().includes(apiKey.toLowerCase())
    )
      return false
    if (
      requestId &&
      !text(row.request_id, "").toLowerCase().includes(requestId.toLowerCase())
    )
      return false
    return true
  })
  return (
    <>
      <PageHeader title="Requests" />
      <Card className="mb-4">
        <CardContent className="grid gap-3 py-4 md:grid-cols-3 xl:grid-cols-6">
          <TimeRangeSelector value={range} onChange={setRange} />
          <Select
            value={status}
            onValueChange={(value) => setStatus(value ?? "all")}
          >
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">All statuses</SelectItem>
              <SelectItem value="succeeded">Succeeded</SelectItem>
              <SelectItem value="provider_failed">Provider failed</SelectItem>
              <SelectItem value="interrupted">Interrupted</SelectItem>
            </SelectContent>
          </Select>
          <Input
            value={provider}
            onChange={(event) => setProvider(event.target.value)}
            placeholder="Provider"
          />
          <Input
            value={model}
            onChange={(event) => setModel(event.target.value)}
            placeholder="Model alias"
          />
          <Input
            value={apiKey}
            onChange={(event) => setApiKey(event.target.value)}
            placeholder="API key"
          />
          <Input
            value={requestId}
            onChange={(event) => setRequestId(event.target.value)}
            placeholder="Request ID"
          />
        </CardContent>
      </Card>
      <Card>
        <CardContent className="overflow-x-auto">
          <DataState
            loading={state.loading}
            error={state.error}
            onRetry={state.reload}
          >
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Timestamp</TableHead>
                  <TableHead>Status</TableHead>
                  <TableHead>API key</TableHead>
                  <TableHead>Alias</TableHead>
                  <TableHead>Provider / model</TableHead>
                  <TableHead>Input</TableHead>
                  <TableHead>Output</TableHead>
                  <TableHead>Cost</TableHead>
                  <TableHead>Latency</TableHead>
                  <TableHead>Request ID</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {!rows.length && (
                  <EmptyTableRow
                    columns={10}
                    title="No matching requests"
                    description="Adjust filters or send an inference request to this project."
                  />
                )}
                {rows.map((row, index) => (
                  <TableRow
                    key={text(row.request_id, String(index))}
                    className="cursor-pointer"
                    onClick={() => setSelected(row)}
                  >
                    <TableCell>{date(row.occurred_at)}</TableCell>
                    <TableCell>
                      <StatusBadge status={text(row.status)} />
                    </TableCell>
                    <TableCell>
                      {text(row.api_key_name ?? row.principal_id)}
                    </TableCell>
                    <TableCell className="font-mono">
                      {text(row.requested_model)}
                    </TableCell>
                    <TableCell>
                      {text(row.provider)} /{" "}
                      <span className="font-mono">
                        {text(row.upstream_model)}
                      </span>
                    </TableCell>
                    <TableCell>
                      {Number(row.prompt_tokens ?? 0).toLocaleString()}
                    </TableCell>
                    <TableCell>
                      {Number(row.completion_tokens ?? 0).toLocaleString()}
                    </TableCell>
                    <TableCell>{money(row.estimated_cost)}</TableCell>
                    <TableCell>
                      {row.latency_ms ? `${row.latency_ms} ms` : "—"}
                    </TableCell>
                    <TableCell className="max-w-36 truncate font-mono">
                      {text(row.request_id)}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </DataState>
        </CardContent>
      </Card>
      <Sheet
        open={Boolean(selected)}
        onOpenChange={(open) => !open && setSelected(undefined)}
      >
        <SheetContent className="overflow-y-auto sm:max-w-xl">
          <SheetHeader>
            <SheetTitle>Request details</SheetTitle>
            <SheetDescription className="font-mono">
              {text(selected?.request_id)}
            </SheetDescription>
          </SheetHeader>
          <div className="space-y-5 p-4 text-sm">
            <section>
              <h3 className="font-semibold">Routing decision</h3>
              <p>
                {text(selected?.requested_model)} → {text(selected?.provider)} /{" "}
                {text(selected?.upstream_model)}
              </p>
              <p className="text-muted-foreground">
                {Number(selected?.attempt_count ?? 1) - 1} retries
              </p>
            </section>
            <section>
              <h3 className="font-semibold">Timing</h3>
              <p>
                {date(selected?.occurred_at)} ·{" "}
                {selected?.latency_ms
                  ? `${selected.latency_ms} ms`
                  : "Latency unavailable"}
              </p>
            </section>
            <section>
              <h3 className="font-semibold">Usage</h3>
              <p>
                {Number(selected?.prompt_tokens ?? 0)} input ·{" "}
                {Number(selected?.completion_tokens ?? 0)} output ·{" "}
                {money(selected?.estimated_cost)}
              </p>
            </section>
            <section>
              <h3 className="font-semibold">Sanitized metadata</h3>
              <p>Operation: {text(selected?.operation, "inference")}</p>
              <p>Status: {text(selected?.status)}</p>
              <p className="mt-2 text-xs text-muted-foreground">
                Prompts, responses, credentials, and arbitrary client metadata
                are not exposed.
              </p>
            </section>
          </div>
        </SheetContent>
      </Sheet>
    </>
  )
}

function humanAudit(row: Row) {
  const payload =
    typeof row.payload === "object" && row.payload ? (row.payload as Row) : {}
  const resource = text(
    payload.resource_kind,
    text(row.resource_type, "resource")
  )
  const id = text(payload.resource_id, "")
  const action = text(
    payload.action,
    text(row.event_type, "changed").split(".").at(-1)
  )
  return {
    actor: text(row.actor_email ?? row.principal_id, "System"),
    action,
    resource,
    summary: `${resource.replaceAll("_", " ")}${id ? ` ${id}` : ""} was ${action}`,
  }
}

export function AuditLogsPage() {
  const session = useGateway()
  const [actor, setActor] = React.useState("")
  const [action, setAction] = React.useState("")
  const [resource, setResource] = React.useState("")
  const [range, setRange] = React.useState<TimeRange>("30d")
  const [selected, setSelected] = React.useState<Row>()
  const state = useGatewayData<Page<Row>>(
    useRangePath(
      "/admin/audit-events",
      session.tenantId,
      session.projectId,
      range,
      { limit: "100" }
    )
  )
  const rows = (state.data?.data ?? []).filter((row) => {
    const readable = humanAudit(row)
    return (
      (!actor || readable.actor.toLowerCase().includes(actor.toLowerCase())) &&
      (!action ||
        readable.action.toLowerCase().includes(action.toLowerCase())) &&
      (!resource ||
        readable.resource.toLowerCase().includes(resource.toLowerCase()))
    )
  })
  return (
    <>
      <PageHeader title="Audit Logs" />
      <Card className="mb-4">
        <CardContent className="grid gap-3 py-4 md:grid-cols-4">
          <TimeRangeSelector value={range} onChange={setRange} />
          <Input
            value={actor}
            onChange={(event) => setActor(event.target.value)}
            placeholder="Actor"
          />
          <Input
            value={action}
            onChange={(event) => setAction(event.target.value)}
            placeholder="Action"
          />
          <Input
            value={resource}
            onChange={(event) => setResource(event.target.value)}
            placeholder="Resource type"
          />
        </CardContent>
      </Card>
      <Card>
        <CardContent>
          <DataState
            loading={state.loading}
            error={state.error}
            onRetry={state.reload}
          >
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Timestamp</TableHead>
                  <TableHead>Actor</TableHead>
                  <TableHead>Action</TableHead>
                  <TableHead>Resource</TableHead>
                  <TableHead>Summary</TableHead>
                  <TableHead>Source / IP</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {!rows.length && (
                  <EmptyTableRow
                    columns={6}
                    title="No audit events"
                    description="Administrative changes will appear here."
                  />
                )}
                {rows.map((row, index) => {
                  const readable = humanAudit(row)
                  return (
                    <TableRow
                      key={text(row.event_id, String(index))}
                      className="cursor-pointer"
                      onClick={() => setSelected(row)}
                    >
                      <TableCell>{date(row.occurred_at)}</TableCell>
                      <TableCell>{readable.actor}</TableCell>
                      <TableCell className="capitalize">
                        {readable.action}
                      </TableCell>
                      <TableCell>
                        {readable.resource.replaceAll("_", " ")}
                      </TableCell>
                      <TableCell>{readable.summary}</TableCell>
                      <TableCell>{text(row.source_ip)}</TableCell>
                    </TableRow>
                  )
                })}
              </TableBody>
            </Table>
          </DataState>
        </CardContent>
      </Card>
      <Sheet
        open={Boolean(selected)}
        onOpenChange={(open) => !open && setSelected(undefined)}
      >
        <SheetContent className="overflow-y-auto">
          <SheetHeader>
            <SheetTitle>Audit event details</SheetTitle>
            <SheetDescription>
              Structured, sanitized event payload.
            </SheetDescription>
          </SheetHeader>
          <pre className="m-4 overflow-auto rounded-md bg-muted p-3 text-xs whitespace-pre-wrap">
            {JSON.stringify(selected, null, 2)}
          </pre>
        </SheetContent>
      </Sheet>
    </>
  )
}

export function PoliciesPage() {
  const session = useGateway()
  const policies = useGatewayData<Page<Resource>>(
    projectPath("/admin/policies", session.tenantId, session.projectId)
  )
  const quotas = useGatewayData<Page<Resource>>(
    projectPath("/admin/quota-limits", session.tenantId, session.projectId)
  )
  const [editing, setEditing] = React.useState<{
    kind: "policy" | "quota"
    value?: Resource
  }>()
  const [retiring, setRetiring] = React.useState<{
    kind: "policy" | "quota"
    value: Resource
  }>()
  const writable = canWrite(session.tenantRole, session.gatewayAdmin)
  async function save(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (!editing) return
    const form = new FormData(event.currentTarget)
    const policy = editing.kind === "policy"
    const path = policy ? "/admin/policies" : "/admin/quota-limits"
    const body = policy
      ? {
          tenant_id: session.tenantId,
          project_id: session.projectId,
          scope_kind: "project",
          scope_id: session.projectId,
          policy: {
            allowed_models: String(form.get("allowed_models") ?? "")
              .split(",")
              .map((v) => v.trim())
              .filter(Boolean),
            allowed_operations: String(form.get("operations") ?? "")
              .split(",")
              .map((v) => v.trim())
              .filter(Boolean),
            max_output_tokens: Number(form.get("max_output_tokens")) || null,
            concurrent_requests: Number(form.get("concurrency")) || null,
          },
        }
      : {
          tenant_id: session.tenantId,
          project_id: session.projectId,
          scope_kind: "project",
          scope_id: session.projectId,
          period: form.get("period"),
          token_limit: Number(form.get("token_limit")) || null,
          cost_limit: Number(form.get("cost_limit")) || null,
          requests_per_minute: Number(form.get("rpm")) || null,
        }
    try {
      await gatewayFetch(
        editing.value ? `${path}/${editing.value.id}` : path,
        session.tenantId,
        {
          method: editing.value ? "PATCH" : "POST",
          headers: {
            "content-type": "application/json",
            ...(editing.value
              ? { "if-match": `"${editing.value.version ?? 1}"` }
              : {}),
          },
          body: JSON.stringify(body),
        }
      )
      setEditing(undefined)
      if (policy) policies.reload()
      else quotas.reload()
    } catch (error) {
      toast.error(
        error instanceof Error ? error.message : "Policy creation failed"
      )
    }
  }
  async function toggle(kind: "policy" | "quota", value: Resource) {
    const path = kind === "policy" ? "/admin/policies" : "/admin/quota-limits"
    const body =
      kind === "policy"
        ? {
            tenant_id: session.tenantId,
            project_id: session.projectId,
            scope_kind: value.scope_kind,
            scope_id: value.scope_id,
            policy: value.policy,
            enabled: value.enabled === false,
          }
        : {
            tenant_id: session.tenantId,
            project_id: session.projectId,
            scope_kind: value.scope_kind,
            scope_id: value.scope_id,
            period: value.period,
            token_limit: value.token_limit,
            cost_limit: value.cost_limit,
            requests_per_minute: value.requests_per_minute,
            enabled: value.enabled === false,
          }
    try {
      await gatewayFetch(`${path}/${value.id}`, session.tenantId, {
        method: "PATCH",
        headers: {
          "content-type": "application/json",
          "if-match": `"${value.version ?? 1}"`,
        },
        body: JSON.stringify(body),
      })
      if (kind === "policy") policies.reload()
      else quotas.reload()
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Update failed")
    }
  }
  async function retire() {
    if (!retiring) return
    const base =
      retiring.kind === "policy" ? "/admin/policies" : "/admin/quota-limits"
    await gatewayFetch(`${base}/${retiring.value.id}`, session.tenantId, {
      method: "DELETE",
      headers: { "if-match": `"${retiring.value.version ?? 1}"` },
    })
    setRetiring(undefined)
    if (retiring.kind === "policy") policies.reload()
    else quotas.reload()
  }
  const sections = [
    ["Model access", policies.data?.data, "Allowed and denied public aliases"],
    ["Rate limits", quotas.data?.data, "Request rate and period limits"],
    ["Token limits", quotas.data?.data, "Token ceilings by period"],
    ["Budgets", quotas.data?.data, "Estimated cost ceilings"],
    ["Concurrency", policies.data?.data, "Simultaneous request limits"],
    ["Operations", policies.data?.data, "Allowed gateway operations"],
    ["Logging", [], "Inherited from Project Settings"],
    ["Retention", [], "Sanitized request traces follow Project Settings"],
  ] as const
  const editingPolicy = (editing?.value?.policy ?? {}) as Row
  return (
    <>
      <PageHeader
        title="Policies"
        action={
          <div className="flex gap-2">
            <Button
              variant="outline"
              disabled={!writable}
              onClick={() => setEditing({ kind: "quota" })}
            >
              Create limit
            </Button>
            <Button
              disabled={!writable}
              onClick={() => setEditing({ kind: "policy" })}
            >
              Create policy
            </Button>
          </div>
        }
      />
      <DataState
        loading={policies.loading || quotas.loading}
        error={policies.error ?? quotas.error}
        onRetry={() => {
          policies.reload()
          quotas.reload()
        }}
      >
        <div className="grid gap-4 md:grid-cols-2">
          {sections.map(([title, source, description]) => {
            const records = source ?? []
            const kind =
              title === "Rate limits" ||
              title === "Token limits" ||
              title === "Budgets"
                ? "quota"
                : "policy"
            return (
              <Card key={title}>
                <CardHeader>
                  <CardTitle className="text-base">{title}</CardTitle>
                  <CardDescription>{description}</CardDescription>
                </CardHeader>
                <CardContent className="space-y-2">
                  {records.length ? (
                    records.map((record) => (
                      <div
                        key={record.id}
                        className="flex items-center justify-between rounded-md border p-3"
                      >
                        <div>
                          <StatusBadge
                            status={
                              record.enabled === false ? "Disabled" : "Active"
                            }
                          />
                          <p className="mt-1 text-xs text-muted-foreground">
                            {text(record.period, "Project scope")} · v
                            {Number(record.version ?? 1)}
                          </p>
                        </div>
                        <div className="flex gap-1">
                          <Button
                            size="sm"
                            variant="outline"
                            disabled={!writable}
                            onClick={() => setEditing({ kind, value: record })}
                          >
                            Edit
                          </Button>
                          <Button
                            size="sm"
                            variant="outline"
                            disabled={!writable}
                            onClick={() => toggle(kind, record)}
                          >
                            {record.enabled === false ? "Enable" : "Disable"}
                          </Button>
                          <Button
                            size="sm"
                            variant="outline"
                            disabled={!writable}
                            onClick={() => setRetiring({ kind, value: record })}
                          >
                            Retire
                          </Button>
                        </div>
                      </div>
                    ))
                  ) : (
                    <p className="text-sm text-muted-foreground">
                      No project override. Organization defaults apply.
                    </p>
                  )}
                </CardContent>
              </Card>
            )
          })}
        </div>
      </DataState>
      <Dialog
        open={Boolean(editing)}
        onOpenChange={(open) => !open && setEditing(undefined)}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>
              {editing?.value ? "Edit" : "Create"} project{" "}
              {editing?.kind === "policy" ? "policy" : "limit"}
            </DialogTitle>
            <DialogDescription>
              Only values entered here override broader policy scopes.
            </DialogDescription>
          </DialogHeader>
          <form onSubmit={save}>
            <FieldGroup>
              {editing?.kind === "policy" ? (
                <>
                  <Field>
                    <FieldLabel>Allowed models</FieldLabel>
                    <Input
                      name="allowed_models"
                      placeholder="alias-a, alias-b"
                      defaultValue={
                        Array.isArray(editingPolicy.allowed_models)
                          ? editingPolicy.allowed_models.join(", ")
                          : ""
                      }
                    />
                  </Field>
                  <Field>
                    <FieldLabel>Allowed operations</FieldLabel>
                    <Input
                      name="operations"
                      placeholder="chat_completion, response, embedding"
                      defaultValue={
                        Array.isArray(editingPolicy.allowed_operations)
                          ? editingPolicy.allowed_operations.join(", ")
                          : ""
                      }
                    />
                  </Field>
                  <Field>
                    <FieldLabel>Max output tokens</FieldLabel>
                    <Input
                      name="max_output_tokens"
                      type="number"
                      min={1}
                      defaultValue={text(editingPolicy.max_output_tokens, "")}
                    />
                  </Field>
                  <Field>
                    <FieldLabel>Concurrency</FieldLabel>
                    <Input
                      name="concurrency"
                      type="number"
                      min={1}
                      defaultValue={text(editingPolicy.concurrent_requests, "")}
                    />
                  </Field>
                </>
              ) : (
                <>
                  <Field>
                    <FieldLabel>Period</FieldLabel>
                    <Select
                      name="period"
                      defaultValue={text(editing?.value?.period, "day")}
                    >
                      <SelectTrigger>
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value="minute">Minute</SelectItem>
                        <SelectItem value="day">Day</SelectItem>
                        <SelectItem value="month">Month</SelectItem>
                      </SelectContent>
                    </Select>
                  </Field>
                  <Field>
                    <FieldLabel>Request limit</FieldLabel>
                    <Input
                      name="rpm"
                      type="number"
                      min={1}
                      defaultValue={text(
                        editing?.value?.requests_per_minute,
                        ""
                      )}
                    />
                  </Field>
                  <Field>
                    <FieldLabel>Token limit</FieldLabel>
                    <Input
                      name="token_limit"
                      type="number"
                      min={1}
                      defaultValue={text(editing?.value?.token_limit, "")}
                    />
                  </Field>
                  <Field>
                    <FieldLabel>Budget</FieldLabel>
                    <Input
                      name="cost_limit"
                      type="number"
                      min={0}
                      step="0.01"
                      defaultValue={text(editing?.value?.cost_limit, "")}
                    />
                  </Field>
                </>
              )}
              <DialogFooter>
                <Button type="submit">
                  {editing?.value ? "Save changes" : `Create ${editing?.kind}`}
                </Button>
              </DialogFooter>
            </FieldGroup>
          </form>
        </DialogContent>
      </Dialog>
      <AlertDialog
        open={Boolean(retiring)}
        onOpenChange={(open) => !open && setRetiring(undefined)}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Retire this policy?</AlertDialogTitle>
            <AlertDialogDescription>
              The broader policy scope will apply immediately. Audit history is
              preserved.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction variant="destructive" onClick={retire}>
              Retire policy
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  )
}

export function IntegrationsPage() {
  const session = useGateway()
  const endpoint = useGatewayEndpoint()
  const locale = usePathname().split("/")[1]
  const webhooks = useGatewayData<Page<Resource>>(
    `/admin/billing/webhooks?tenant_id=${encodeURIComponent(session.tenantId)}`
  )
  const outbox = useGatewayData<Page<Resource>>(
    `/admin/billing/outbox?tenant_id=${encodeURIComponent(session.tenantId)}`
  )
  const system = useGatewayData<Row>(
    `/admin/system?tenant_id=${encodeURIComponent(session.tenantId)}`
  )
  async function retry(eventId: unknown) {
    await gatewayFetch(
      `/admin/billing/outbox/${eventId}/retry`,
      session.tenantId,
      { method: "POST" }
    )
    outbox.reload()
  }
  return (
    <>
      <PageHeader
        title="Integrations"
        action={
          <Button
            variant="outline"
            render={<Link href={`/${locale}/${session.tenantId}/billing`} />}
          >
            Configure organization webhooks
          </Button>
        }
      />
      <div className="grid gap-6 xl:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>Webhooks</CardTitle>
            <CardDescription>
              Billing and usage delivery endpoints.
            </CardDescription>
          </CardHeader>
          <CardContent>
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Endpoint</TableHead>
                  <TableHead>Events</TableHead>
                  <TableHead>Status</TableHead>
                  <TableHead>Last delivery</TableHead>
                  <TableHead>Failures</TableHead>
                  <TableHead />
                </TableRow>
              </TableHeader>
              <TableBody>
                {!webhooks.data?.data.length && (
                  <EmptyTableRow
                    columns={6}
                    title="No webhooks configured"
                    description="Webhook management is available at organization scope."
                  />
                )}
                {webhooks.data?.data.map((row) => {
                  const deliveries = (outbox.data?.data ?? []).filter(
                    (event) => event.webhook_id === row.webhook_id
                  )
                  const latest = deliveries[0]
                  return (
                    <TableRow key={row.id ?? text(row.webhook_id)}>
                      <TableCell className="max-w-52 truncate">
                        {text(row.url)}
                      </TableCell>
                      <TableCell>Usage and billing</TableCell>
                      <TableCell>
                        <StatusBadge
                          status={row.enabled === false ? "Disabled" : "Active"}
                        />
                      </TableCell>
                      <TableCell>{date(latest?.delivered_at)}</TableCell>
                      <TableCell>
                        {deliveries.filter((event) => event.last_error).length}
                      </TableCell>
                      <TableCell>
                        <Button
                          size="sm"
                          variant="outline"
                          render={
                            <Link
                              href={`/${locale}/${session.tenantId}/billing`}
                            />
                          }
                        >
                          Manage
                        </Button>
                      </TableCell>
                    </TableRow>
                  )
                })}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle>Outbox</CardTitle>
            <CardDescription>
              Non-blocking delivery and retry status.
            </CardDescription>
          </CardHeader>
          <CardContent>
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Event</TableHead>
                  <TableHead>Status</TableHead>
                  <TableHead>Attempts</TableHead>
                  <TableHead>Next retry</TableHead>
                  <TableHead />
                </TableRow>
              </TableHeader>
              <TableBody>
                {!outbox.data?.data.length && (
                  <EmptyTableRow
                    columns={5}
                    title="No deliveries"
                    description="Delivery attempts will appear after a subscribed event."
                  />
                )}
                {outbox.data?.data.map((row) => (
                  <TableRow key={text(row.event_id)}>
                    <TableCell className="max-w-32 truncate font-mono">
                      {text(row.event_id)}
                    </TableCell>
                    <TableCell>
                      <StatusBadge
                        status={
                          row.delivered_at
                            ? "Succeeded"
                            : row.last_error
                              ? "Failed"
                              : "Pending"
                        }
                      />
                    </TableCell>
                    <TableCell>{Number(row.attempt_count ?? 0)}</TableCell>
                    <TableCell>{date(row.next_attempt_at)}</TableCell>
                    <TableCell>
                      <Button
                        size="sm"
                        variant="outline"
                        disabled={Boolean(row.delivered_at)}
                        onClick={() => retry(row.event_id)}
                      >
                        <ArrowClockwiseIcon data-icon="inline-start" />
                        Retry
                      </Button>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle>Observability</CardTitle>
            <CardDescription>
              OpenTelemetry and Prometheus export status.
            </CardDescription>
          </CardHeader>
          <CardContent className="flex items-center justify-between">
            <div>
              <p className="font-medium">OpenTelemetry / OTLP</p>
              <p className="text-sm text-muted-foreground">
                Configured by the gateway deployment environment.
              </p>
            </div>
            <StatusBadge
              status={system.data?.otel_enabled ? "Active" : "Not configured"}
            />
          </CardContent>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle>SDK & framework</CardTitle>
            <CardDescription>
              Use any OpenAI-compatible client with project aliases.
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-3">
            <div>
              <p className="text-xs text-muted-foreground">Base URL</p>
              <code className="text-sm">{endpoint}</code>
            </div>
            <p className="text-sm">
              Set the client API key to a Tunel project key and the model to an
              alias from Models.
            </p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle>Notifications</CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-sm text-muted-foreground">
              No notification provider is configured for this deployment.
            </p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle>Billing</CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-sm text-muted-foreground">
              Billing delivery uses the webhook and outbox records shown above.
            </p>
          </CardContent>
        </Card>
      </div>
    </>
  )
}

export function ProjectSettingsPage() {
  const session = useGateway()
  const endpoint = useGatewayEndpoint()
  const router = useRouter()
  const locale = usePathname().split("/")[1]
  const state = useGatewayData<Page<Resource>>(
    projectPath("/admin/projects", session.tenantId, session.projectId)
  )
  const routes = useGatewayData<Page<Resource>>(
    projectPath("/admin/model-routes", session.tenantId, session.projectId)
  )
  const [pending, setPending] = React.useState(false)
  const [archiving, setArchiving] = React.useState(false)
  const [confirmation, setConfirmation] = React.useState("")
  const [selectedDefaultAlias, setSelectedDefaultAlias] =
    React.useState<string>()
  const project = state.data?.data[0]
  const defaultAlias =
    selectedDefaultAlias ??
    text(
      project?.default_alias,
      text(routes.data?.data[0]?.requested_model, "")
    )
  const writable = canWrite(session.tenantRole, session.gatewayAdmin)
  async function save(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (!project) return
    const form = new FormData(event.currentTarget)
    setPending(true)
    try {
      await gatewayFetch(`/admin/projects/${project.id}`, session.tenantId, {
        method: "PATCH",
        headers: {
          "content-type": "application/json",
          "if-match": `"${project.version ?? 1}"`,
        },
        body: JSON.stringify({
          name: form.get("name"),
          slug: form.get("slug"),
          environment: form.get("environment"),
          gateway_timeout_ms: Number(form.get("gateway_timeout_ms")),
          default_alias: form.get("default_alias"),
          logging_mode: form.get("logging_mode"),
          request_retention_days: Number(form.get("request_retention_days")),
        }),
      })
      state.reload()
      toast.success("Project settings saved")
    } catch (error) {
      toast.error(
        error instanceof Error ? error.message : "Settings update failed"
      )
    } finally {
      setPending(false)
    }
  }
  async function archive() {
    if (!project) return
    setPending(true)
    try {
      await gatewayFetch(`/admin/projects/${project.id}`, session.tenantId, {
        method: "DELETE",
        headers: { "if-match": `"${project.version ?? 1}"` },
      })
      router.replace(`/${locale}/${session.tenantId}/projects`)
      router.refresh()
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Archive failed")
      setPending(false)
    }
  }
  return (
    <>
      <PageHeader title="Project Settings" />
      <DataState
        loading={state.loading}
        error={state.error}
        onRetry={state.reload}
      >
        {project && (
          <form onSubmit={save} className="space-y-4">
            <Card>
              <CardHeader>
                <CardTitle>General</CardTitle>
              </CardHeader>
              <CardContent className="grid gap-4 md:grid-cols-2">
                <Field>
                  <FieldLabel>Name</FieldLabel>
                  <Input
                    name="name"
                    defaultValue={text(project.name, "")}
                    required
                    disabled={!writable}
                  />
                </Field>
                <Field>
                  <FieldLabel>Slug</FieldLabel>
                  <Input
                    name="slug"
                    defaultValue={text(project.slug, project.id.slice(0, 8))}
                    pattern="[a-z0-9]+(?:-[a-z0-9]+)*"
                    required
                    disabled={!writable}
                  />
                </Field>
              </CardContent>
            </Card>
            <Card>
              <CardHeader>
                <CardTitle>Environment</CardTitle>
                <CardDescription>
                  The public gateway endpoint is deployment-managed and
                  read-only.
                </CardDescription>
              </CardHeader>
              <CardContent className="grid gap-4 md:grid-cols-2">
                <Field>
                  <FieldLabel>Environment</FieldLabel>
                  <Select
                    name="environment"
                    defaultValue={text(project.environment, "production")}
                    disabled={!writable}
                  >
                    <SelectTrigger>
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="production">Production</SelectItem>
                      <SelectItem value="development">Development</SelectItem>
                      <SelectItem value="staging">Staging</SelectItem>
                    </SelectContent>
                  </Select>
                </Field>
                <Field>
                  <FieldLabel>Gateway base URL</FieldLabel>
                  <Input value={endpoint} readOnly />
                </Field>
              </CardContent>
            </Card>
            <Card>
              <CardHeader>
                <CardTitle>Gateway</CardTitle>
              </CardHeader>
              <CardContent className="grid gap-4 md:grid-cols-3">
                <Field>
                  <FieldLabel>Timeout (ms)</FieldLabel>
                  <Input
                    name="gateway_timeout_ms"
                    type="number"
                    min={1000}
                    max={600000}
                    defaultValue={Number(project.gateway_timeout_ms ?? 120000)}
                    disabled={!writable}
                  />
                </Field>
                <Field>
                  <FieldLabel>Default alias</FieldLabel>
                  <Select
                    name="default_alias"
                    value={defaultAlias}
                    onValueChange={(value) =>
                      value && setSelectedDefaultAlias(value)
                    }
                    disabled={!writable}
                  >
                    <SelectTrigger>
                      <SelectValue placeholder="Select alias" />
                    </SelectTrigger>
                    <SelectContent>
                      {[
                        ...new Set(
                          (routes.data?.data ?? []).map((route) =>
                            text(route.requested_model)
                          )
                        ),
                      ].map((alias) => (
                        <SelectItem key={alias} value={alias}>
                          {alias}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </Field>
                <Field>
                  <FieldLabel>Logging</FieldLabel>
                  <Select
                    name="logging_mode"
                    defaultValue={text(project.logging_mode, "metadata")}
                    disabled={!writable}
                  >
                    <SelectTrigger>
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="minimal">Minimal</SelectItem>
                      <SelectItem value="metadata">
                        Sanitized metadata
                      </SelectItem>
                      <SelectItem value="disabled">Disabled</SelectItem>
                    </SelectContent>
                  </Select>
                </Field>
              </CardContent>
            </Card>
            <Card>
              <CardHeader>
                <CardTitle>Data Retention</CardTitle>
                <CardDescription>
                  Applies to sanitized request traces only; immutable usage,
                  billing, and audit records remain durable.
                </CardDescription>
              </CardHeader>
              <CardContent>
                <Field>
                  <FieldLabel>Request trace retention (days)</FieldLabel>
                  <Input
                    name="request_retention_days"
                    type="number"
                    min={1}
                    max={365}
                    defaultValue={Number(project.request_retention_days ?? 30)}
                    disabled={!writable}
                  />
                </Field>
              </CardContent>
            </Card>
            <Card>
              <CardHeader>
                <CardTitle>Access</CardTitle>
              </CardHeader>
              <CardContent>
                <p className="text-sm text-muted-foreground">
                  Project access inherits organization membership. Owners and
                  administrators can mutate configuration; engineers are
                  read-only where permitted.
                </p>
              </CardContent>
            </Card>
            {writable && (
              <div className="flex justify-end">
                <Button type="submit" disabled={pending}>
                  {pending ? "Saving…" : "Save changes"}
                </Button>
              </div>
            )}
          </form>
        )}
      </DataState>
      <Card className="mt-6 border-destructive/40">
        <CardHeader>
          <CardTitle>Danger Zone</CardTitle>
          <CardDescription>
            Archive this project while preserving usage and audit history.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <Button
            variant="destructive"
            disabled={!writable}
            onClick={() => setArchiving(true)}
          >
            Archive project
          </Button>
        </CardContent>
      </Card>
      <AlertDialog
        open={archiving}
        onOpenChange={(open) => {
          if (!open) {
            setArchiving(false)
            setConfirmation("")
          }
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Archive project?</AlertDialogTitle>
            <AlertDialogDescription>
              Type {text(project?.slug, project?.id)} to confirm. Active keys
              and routing configuration will stop being available.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <Input
            value={confirmation}
            onChange={(event) => setConfirmation(event.target.value)}
            placeholder={text(project?.slug, project?.id)}
          />
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction
              variant="destructive"
              disabled={confirmation !== text(project?.slug, project?.id)}
              onClick={archive}
            >
              Archive project
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  )
}
