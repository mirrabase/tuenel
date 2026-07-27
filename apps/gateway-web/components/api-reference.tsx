"use client"

import * as React from "react"
import {
  CheckIcon,
  CodeIcon,
  CopyIcon,
  DownloadSimpleIcon,
  FileCodeIcon,
  MagnifyingGlassIcon,
  PaperPlaneTiltIcon,
  PlugsConnectedIcon,
} from "@phosphor-icons/react"
import { toast } from "sonner"

import { useGatewayEndpoint } from "@/components/pages/shared"
import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { Textarea } from "@/components/ui/textarea"
import { cn } from "@/lib/utils"

type JsonRecord = Record<string, unknown>

export type ApiEndpoint = {
  id: string
  method: "get" | "post" | "put" | "patch" | "delete"
  path: string
  tag: string
  summary: string
  description?: string
  parameters?: Array<{
    name: string
    in: "query" | "path" | "header"
    required?: boolean
    description?: string
    schema?: JsonRecord
  }>
  requestBody?: {
    required?: boolean
    content?: Record<string, { schema?: JsonRecord; example?: unknown }>
  }
  responses?: Record<
    string,
    {
      description?: string
      content?: Record<string, { schema?: JsonRecord; example?: unknown }>
    }
  >
}

const fallbackEndpoints: ApiEndpoint[] = [
  {
    id: "chat-completions",
    method: "post",
    path: "/v1/chat/completions",
    tag: "Inference",
    summary: "Create chat completion",
    description:
      "Send a model inference request following the OpenAI Chat Completions API format.",
    parameters: [
      {
        name: "Idempotency-Key",
        in: "header",
        required: false,
        description: "Unique string to prevent duplicate execution.",
      },
    ],
    requestBody: {
      required: true,
      content: {
        "application/json": {
          example: {
            model: "gpt-4o",
            messages: [{ role: "user", content: "Hello Tuenel!" }],
            temperature: 0.7,
            stream: false,
          },
        },
      },
    },
    responses: {
      "200": {
        description: "Successful response with assistant completion.",
        content: {
          "application/json": {
            example: {
              id: "chatcmpl-123",
              object: "chat.completion",
              created: 1720000000,
              model: "gpt-4o",
              choices: [
                {
                  index: 0,
                  message: {
                    role: "assistant",
                    content: "Hello! How can I assist you today?",
                  },
                  finish_reason: "stop",
                },
              ],
              usage: {
                prompt_tokens: 12,
                completion_tokens: 9,
                total_tokens: 21,
              },
            },
          },
        },
      },
      "401": { description: "Invalid virtual key or unauthorized access." },
      "429": { description: "Rate limit or budget threshold exceeded." },
    },
  },
  {
    id: "create-response",
    method: "post",
    path: "/v1/responses",
    tag: "Inference",
    summary: "Create unified response",
    description:
      "Send an instruction prompt to receive model output using the unified response format.",
    requestBody: {
      required: true,
      content: {
        "application/json": {
          example: {
            model: "gpt-4o",
            input: [{ role: "user", content: "Explain quantum computing." }],
            stream: false,
          },
        },
      },
    },
    responses: {
      "200": {
        description: "Output generated successfully.",
      },
    },
  },
  {
    id: "embeddings",
    method: "post",
    path: "/v1/embeddings",
    tag: "Inference",
    summary: "Create text embeddings",
    description: "Generate vector embeddings for input text strings.",
    requestBody: {
      required: true,
      content: {
        "application/json": {
          example: {
            model: "text-embedding-3-small",
            input: "Sample query text for embedding",
          },
        },
      },
    },
    responses: {
      "200": { description: "Vector embeddings generated successfully." },
    },
  },
  {
    id: "list-models",
    method: "get",
    path: "/v1/models",
    tag: "Models",
    summary: "List model aliases",
    description: "Retrieve all active model aliases configured in Tuenel.",
    responses: {
      "200": {
        description: "List of available models.",
        content: {
          "application/json": {
            example: {
              object: "list",
              data: [
                { id: "gpt-4o", object: "model", created: 1720000000 },
                {
                  id: "claude-3-5-sonnet",
                  object: "model",
                  created: 1720000000,
                },
              ],
            },
          },
        },
      },
    },
  },
  {
    id: "list-keys",
    method: "get",
    path: "/admin/virtual-keys",
    tag: "Virtual Keys",
    summary: "List virtual keys",
    description: "Fetch virtual API keys for the current project.",
    parameters: [
      { name: "tenant_id", in: "query", required: true },
      { name: "project_id", in: "query", required: true },
    ],
    responses: {
      "200": { description: "List of virtual keys." },
    },
  },
  {
    id: "create-key",
    method: "post",
    path: "/admin/virtual-keys",
    tag: "Virtual Keys",
    summary: "Create virtual key",
    description: "Issue a new virtual API key with optional budget limits.",
    requestBody: {
      required: true,
      content: {
        "application/json": {
          example: {
            name: "Production App Key",
            monthly_budget: 100,
          },
        },
      },
    },
    responses: {
      "201": { description: "Virtual key created successfully." },
    },
  },
  {
    id: "list-routes",
    method: "get",
    path: "/admin/model-routes",
    tag: "Model Routes",
    summary: "List model routes",
    description: "Retrieve configured model routing topologies.",
    responses: {
      "200": { description: "List of model routes." },
    },
  },
  {
    id: "system-health",
    method: "get",
    path: "/admin/system",
    tag: "System",
    summary: "Get system status",
    description: "Check health status of gateway runtime and providers.",
    responses: {
      "200": { description: "System status details." },
    },
  },
]

function MethodBadge({ method }: { method: string }) {
  const m = method.toUpperCase()
  const colorMap: Record<string, string> = {
    GET: "bg-blue-500/10 text-blue-500 border-blue-500/20",
    POST: "bg-emerald-500/10 text-emerald-500 border-emerald-500/20",
    PUT: "bg-amber-500/10 text-amber-500 border-amber-500/20",
    PATCH: "bg-purple-500/10 text-purple-500 border-purple-500/20",
    DELETE: "bg-rose-500/10 text-rose-500 border-rose-500/20",
  }
  return (
    <span
      className={cn(
        "inline-flex shrink-0 items-center rounded border px-2 py-0.5 font-mono text-[10px] font-bold tracking-wider uppercase",
        colorMap[m] || "bg-muted text-muted-foreground"
      )}
    >
      {m}
    </span>
  )
}

function parseOpenApiEndpoints(spec?: JsonRecord): {
  endpoints: ApiEndpoint[]
  schemas: Record<string, JsonRecord>
} {
  if (!spec || typeof spec !== "object" || !spec.paths) {
    return { endpoints: fallbackEndpoints, schemas: {} }
  }

  const paths = spec.paths as Record<string, Record<string, JsonRecord>>
  const parsed: ApiEndpoint[] = []
  const schemas = ((spec.components as JsonRecord)?.schemas ?? {}) as Record<
    string,
    JsonRecord
  >

  Object.entries(paths).forEach(([path, methods]) => {
    Object.entries(methods).forEach(([method, detail]) => {
      if (
        !["get", "post", "put", "patch", "delete"].includes(
          method.toLowerCase()
        )
      ) {
        return
      }
      const tags = Array.isArray(detail.tags) ? (detail.tags as string[]) : []
      const tag = tags[0] || (path.startsWith("/v1") ? "Inference" : "Admin")

      parsed.push({
        id: `${method}-${path.replace(/[/_{}]/g, "-")}`,
        method: method.toLowerCase() as ApiEndpoint["method"],
        path,
        tag,
        summary: String(detail.summary || `${method.toUpperCase()} ${path}`),
        description: detail.description
          ? String(detail.description)
          : undefined,
        parameters: Array.isArray(detail.parameters)
          ? (detail.parameters as ApiEndpoint["parameters"])
          : undefined,
        requestBody: detail.requestBody as ApiEndpoint["requestBody"],
        responses: detail.responses as ApiEndpoint["responses"],
      })
    })
  })

  return {
    endpoints: parsed.length ? parsed : fallbackEndpoints,
    schemas,
  }
}

export function ApiReferenceView({ specData }: { specData?: JsonRecord }) {
  const gatewayEndpoint = useGatewayEndpoint()
  const { endpoints, schemas } = React.useMemo(
    () => parseOpenApiEndpoints(specData),
    [specData]
  )

  const [search, setSearch] = React.useState("")
  const [selectedEndpointId, setSelectedEndpointId] = React.useState<string>(
    endpoints[0]?.id || ""
  )
  const [rawOpenApiModal, setRawOpenApiModal] = React.useState(false)
  const [testPayload, setTestPayload] = React.useState(() =>
    JSON.stringify(
      endpoints[0]?.requestBody?.content?.["application/json"]?.example ?? {},
      null,
      2
    )
  )
  const [testResponse, setTestResponse] = React.useState<string | null>(null)
  const [executing, setExecuting] = React.useState(false)

  const filteredEndpoints = React.useMemo(() => {
    if (!search.trim()) return endpoints
    const q = search.toLowerCase()
    return endpoints.filter(
      (item) =>
        item.path.toLowerCase().includes(q) ||
        item.summary.toLowerCase().includes(q) ||
        item.tag.toLowerCase().includes(q)
    )
  }, [endpoints, search])

  const grouped = React.useMemo(() => {
    const map = new Map<string, ApiEndpoint[]>()
    filteredEndpoints.forEach((item) => {
      const list = map.get(item.tag) || []
      list.push(item)
      map.set(item.tag, list)
    })
    return map
  }, [filteredEndpoints])

  const activeEndpoint =
    endpoints.find((item) => item.id === selectedEndpointId) || endpoints[0]

  async function handleSendRequest() {
    if (!activeEndpoint) return
    setExecuting(true)
    setTestResponse(null)
    const startTime = performance.now()

    try {
      const url = `${gatewayEndpoint}${activeEndpoint.path}`
      const init: RequestInit = {
        method: activeEndpoint.method.toUpperCase(),
        headers: {
          "Content-Type": "application/json",
          Authorization: "Bearer TVK_DEMO_KEY",
        },
      }
      if (
        ["post", "put", "patch"].includes(activeEndpoint.method) &&
        testPayload.trim()
      ) {
        init.body = testPayload
      }

      const res = await fetch(url, init)
      const duration = Math.round(performance.now() - startTime)
      let data: unknown
      try {
        data = await res.json()
      } catch {
        data = await res.text()
      }

      setTestResponse(
        JSON.stringify(
          {
            status: res.status,
            statusText: res.statusText,
            durationMs: duration,
            headers: Object.fromEntries(res.headers.entries()),
            body: data,
          },
          null,
          2
        )
      )
      toast.success(`Request completed in ${duration}ms`)
    } catch (err) {
      setTestResponse(
        JSON.stringify(
          {
            error: err instanceof Error ? err.message : "Request failed",
          },
          null,
          2
        )
      )
      toast.error("Execution error")
    } finally {
      setExecuting(false)
    }
  }

  function downloadRawSpec() {
    const blob = new Blob(
      [JSON.stringify(specData || fallbackEndpoints, null, 2)],
      {
        type: "application/json",
      }
    )
    const url = URL.createObjectURL(blob)
    const a = document.createElement("a")
    a.href = url
    a.download = "tuenel-openapi.json"
    a.click()
    URL.revokeObjectURL(url)
    toast.success("OpenAPI JSON downloaded")
  }

  return (
    <div className="flex h-[calc(100vh-6.5rem)] min-w-0 flex-col overflow-hidden rounded-xl border bg-background shadow-sm">
      {/* Top Bar */}
      <div className="flex shrink-0 flex-wrap items-center justify-between gap-3 border-b bg-card px-4 py-3">
        <div className="flex items-center gap-3">
          <div className="flex size-8 items-center justify-center rounded-lg bg-primary/10 text-primary">
            <CodeIcon className="size-4" />
          </div>
          <div>
            <h2 className="text-sm font-semibold tracking-tight">
              API Reference
            </h2>
            <p className="font-mono text-[11px] text-muted-foreground">
              {gatewayEndpoint}/v1
            </p>
          </div>
        </div>

        <div className="flex items-center gap-2">
          <Button
            size="sm"
            variant="outline"
            className="h-8 gap-1.5 text-xs"
            onClick={() => setRawOpenApiModal(true)}
          >
            <FileCodeIcon className="size-3.5" />
            OpenAPI Spec
          </Button>
          <Button
            size="sm"
            variant="outline"
            className="h-8 gap-1.5 text-xs"
            onClick={downloadRawSpec}
          >
            <DownloadSimpleIcon className="size-3.5" />
            Download JSON
          </Button>
        </div>
      </div>

      {/* Main Split Layout */}
      <div className="grid min-h-0 flex-1 lg:grid-cols-[280px_1fr]">
        {/* Left Navigation Panel */}
        <div className="flex min-h-0 flex-col border-b bg-card/40 lg:border-r lg:border-b-0">
          <div className="p-3">
            <div className="relative">
              <MagnifyingGlassIcon className="absolute top-2.5 left-2.5 size-3.5 text-muted-foreground" />
              <Input
                placeholder="Search endpoints…"
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                className="h-8 pl-8 text-xs"
              />
            </div>
          </div>

          <div className="min-h-0 flex-1 space-y-4 overflow-y-auto px-2 pb-4">
            {Array.from(grouped.entries()).map(([tag, list]) => (
              <div key={tag} className="space-y-1">
                <div className="px-2 text-[10px] font-semibold tracking-wider text-muted-foreground uppercase">
                  {tag}
                </div>
                {list.map((item) => {
                  const active = item.id === activeEndpoint?.id
                  return (
                    <button
                      key={item.id}
                      type="button"
                      onClick={() => {
                        setSelectedEndpointId(item.id)
                        setTestPayload(
                          JSON.stringify(
                            item.requestBody?.content?.["application/json"]
                              ?.example ?? {},
                            null,
                            2
                          )
                        )
                        setTestResponse(null)
                      }}
                      className={cn(
                        "flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-xs transition-colors",
                        active
                          ? "bg-accent font-medium text-foreground shadow-xs"
                          : "text-muted-foreground hover:bg-accent/50 hover:text-foreground"
                      )}
                    >
                      <MethodBadge method={item.method} />
                      <span className="truncate font-mono text-[11px]">
                        {item.path}
                      </span>
                    </button>
                  )
                })}
              </div>
            ))}
          </div>
        </div>

        {/* Right Content Area: Endpoint Docs + Try It Interactive Panel */}
        {activeEndpoint && (
          <div className="grid min-h-0 flex-1 divide-y overflow-y-auto lg:grid-cols-12 lg:divide-x lg:divide-y-0">
            {/* Left: Endpoint Spec Documentation */}
            <div className="space-y-6 p-6 lg:col-span-7">
              <div>
                <div className="flex items-center gap-2">
                  <MethodBadge method={activeEndpoint.method} />
                  <span className="font-mono text-sm font-semibold">
                    {activeEndpoint.path}
                  </span>
                  <Button
                    size="icon-xs"
                    variant="ghost"
                    onClick={() =>
                      navigator.clipboard
                        .writeText(`${gatewayEndpoint}${activeEndpoint.path}`)
                        .then(() => toast.success("Endpoint URL copied"))
                    }
                  >
                    <CopyIcon className="size-3" />
                  </Button>
                </div>
                <h1 className="mt-2 text-lg font-bold">
                  {activeEndpoint.summary}
                </h1>
                {activeEndpoint.description && (
                  <p className="mt-1 text-xs leading-relaxed text-muted-foreground">
                    {activeEndpoint.description}
                  </p>
                )}
              </div>

              {/* Security / Auth */}
              <div className="space-y-2">
                <h3 className="text-xs font-semibold tracking-wider text-muted-foreground uppercase">
                  Authentication
                </h3>
                <div className="inline-flex items-center gap-2 rounded-md border bg-muted/30 px-3 py-1.5 font-mono text-xs">
                  <span className="size-2 rounded-full bg-emerald-500" />
                  Bearer Virtual Key (
                  <code className="text-foreground">TVK_...</code>)
                </div>
              </div>

              {/* Parameters */}
              {activeEndpoint.parameters &&
                activeEndpoint.parameters.length > 0 && (
                  <div className="space-y-3">
                    <h3 className="text-xs font-semibold tracking-wider text-muted-foreground uppercase">
                      Parameters
                    </h3>
                    <div className="divide-y rounded-md border text-xs">
                      {activeEndpoint.parameters.map((param) => (
                        <div
                          key={param.name}
                          className="flex flex-col gap-1 p-3 sm:flex-row sm:items-center sm:justify-between"
                        >
                          <div className="flex items-center gap-2">
                            <span className="font-mono font-semibold">
                              {param.name}
                            </span>
                            <span className="rounded bg-muted px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground">
                              {param.in}
                            </span>
                            {param.required && (
                              <span className="text-[10px] font-medium text-rose-500">
                                required
                              </span>
                            )}
                          </div>
                          {param.description && (
                            <span className="text-muted-foreground">
                              {param.description}
                            </span>
                          )}
                        </div>
                      ))}
                    </div>
                  </div>
                )}

              {/* Request Body Example */}
              {activeEndpoint.requestBody && (
                <div className="space-y-2">
                  <h3 className="text-xs font-semibold tracking-wider text-muted-foreground uppercase">
                    Request Body
                  </h3>
                  <div className="rounded-lg border bg-zinc-950 p-3 text-zinc-50 dark:bg-zinc-900">
                    <pre className="max-h-48 overflow-auto font-mono text-xs">
                      {JSON.stringify(
                        activeEndpoint.requestBody.content?.["application/json"]
                          ?.example || {},
                        null,
                        2
                      )}
                    </pre>
                  </div>
                </div>
              )}

              {/* Response Codes */}
              {activeEndpoint.responses && (
                <div className="space-y-3">
                  <h3 className="text-xs font-semibold tracking-wider text-muted-foreground uppercase">
                    Responses
                  </h3>
                  <div className="space-y-2 text-xs">
                    {Object.entries(activeEndpoint.responses).map(
                      ([code, detail]) => (
                        <div
                          key={code}
                          className="space-y-2 rounded-md border p-3"
                        >
                          <div className="flex items-center justify-between">
                            <span className="font-mono font-bold text-emerald-500">
                              {code}
                            </span>
                            <span className="text-muted-foreground">
                              {detail.description}
                            </span>
                          </div>
                          {Boolean(
                            detail.content?.["application/json"]?.example
                          ) && (
                            <pre className="max-h-36 overflow-auto rounded bg-zinc-950 p-2 font-mono text-[11px] text-zinc-200">
                              {JSON.stringify(
                                detail.content?.["application/json"]?.example,
                                null,
                                2
                              )}
                            </pre>
                          )}
                        </div>
                      )
                    )}
                  </div>
                </div>
              )}

              {/* Collapsible Schemas Section */}
              {Object.keys(schemas).length > 0 && (
                <div className="space-y-3 border-t pt-4">
                  <h3 className="text-xs font-semibold tracking-wider text-muted-foreground uppercase">
                    Schema Models
                  </h3>
                  <details className="group rounded-md border">
                    <summary className="flex cursor-pointer items-center justify-between p-3 text-xs font-medium hover:bg-muted/40">
                      View OpenAPI Component Schemas (
                      {Object.keys(schemas).length})
                      <span className="text-muted-foreground transition-transform group-open:rotate-90">
                        ›
                      </span>
                    </summary>
                    <div className="max-h-80 space-y-4 overflow-y-auto border-t p-3">
                      {Object.entries(schemas).map(([name, schema]) => (
                        <div key={name} className="space-y-1">
                          <h4 className="font-mono text-xs font-semibold text-primary">
                            {name}
                          </h4>
                          <pre className="rounded bg-muted/40 p-2 font-mono text-[11px] whitespace-pre-wrap">
                            {JSON.stringify(schema, null, 2)}
                          </pre>
                        </div>
                      ))}
                    </div>
                  </details>
                </div>
              )}
            </div>

            {/* Right: Interactive Try It / Send Request Panel */}
            <div className="space-y-5 bg-card/20 p-6 lg:col-span-5">
              <div className="flex items-center justify-between">
                <h3 className="text-xs font-semibold tracking-wider text-muted-foreground uppercase">
                  Try It (Interactive)
                </h3>
                <Button
                  size="sm"
                  disabled={executing}
                  onClick={handleSendRequest}
                  className="gap-1.5 text-xs"
                >
                  <PaperPlaneTiltIcon className="size-3.5" />
                  {executing ? "Sending…" : "Send Request"}
                </Button>
              </div>

              <div className="space-y-3 text-xs">
                <div>
                  <span className="text-muted-foreground">Target URL</span>
                  <div className="mt-1 truncate rounded-md border bg-muted/30 p-2 font-mono text-[11px]">
                    {gatewayEndpoint}
                    {activeEndpoint.path}
                  </div>
                </div>

                {["post", "put", "patch"].includes(activeEndpoint.method) && (
                  <div className="space-y-1">
                    <span className="text-muted-foreground">
                      Request Payload (JSON)
                    </span>
                    <Textarea
                      rows={8}
                      value={testPayload}
                      onChange={(e) => setTestPayload(e.target.value)}
                      className="font-mono text-xs leading-relaxed"
                    />
                  </div>
                )}
              </div>

              {testResponse && (
                <div className="space-y-2">
                  <h4 className="text-xs font-semibold tracking-wider text-muted-foreground uppercase">
                    Response Output
                  </h4>
                  <div className="rounded-lg border bg-zinc-950 p-3 text-zinc-50 dark:bg-zinc-900">
                    <pre className="max-h-64 overflow-auto font-mono text-xs leading-relaxed">
                      {testResponse}
                    </pre>
                  </div>
                </div>
              )}
            </div>
          </div>
        )}
      </div>

      {/* Raw OpenAPI Spec Modal */}
      <Dialog open={rawOpenApiModal} onOpenChange={setRawOpenApiModal}>
        <DialogContent className="max-w-3xl">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2">
              <FileCodeIcon className="size-5 text-primary" />
              Raw OpenAPI Specification
            </DialogTitle>
            <DialogDescription>
              Complete OpenAPI 3.0 specification for Tuenel Gateway.
            </DialogDescription>
          </DialogHeader>

          <div className="py-2">
            <pre className="max-h-[60vh] overflow-auto rounded-lg border bg-zinc-950 p-4 font-mono text-xs text-zinc-50 dark:bg-zinc-900">
              {JSON.stringify(specData || fallbackEndpoints, null, 2)}
            </pre>
          </div>

          <div className="flex justify-between">
            <Button size="sm" variant="outline" onClick={downloadRawSpec}>
              <DownloadSimpleIcon className="mr-1.5 size-3.5" />
              Download JSON
            </Button>
            <Button size="sm" onClick={() => setRawOpenApiModal(false)}>
              Close
            </Button>
          </div>
        </DialogContent>
      </Dialog>
    </div>
  )
}
