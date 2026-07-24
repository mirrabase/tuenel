"use client"

import * as React from "react"
import {
  ArrowCounterClockwiseIcon,
  ArrowDownIcon,
  ArrowUpIcon,
  CodeIcon,
  CopyIcon,
  DownloadSimpleIcon,
  PaperPlaneTiltIcon,
  PencilSimpleIcon,
  PlusIcon,
  SidebarSimpleIcon,
  StopIcon,
  TrashIcon,
  WrenchIcon,
} from "@phosphor-icons/react"
import { toast } from "sonner"

import { useGateway } from "@/components/gateway-provider"
import { useGatewayData, useGatewayEndpoint } from "@/components/pages/shared"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select"
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet"
import { Spinner } from "@/components/ui/spinner"
import { Switch } from "@/components/ui/switch"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { Textarea } from "@/components/ui/textarea"
import {
  GatewayApiError,
  type Page,
  gatewayResponse,
  readSse,
} from "@/lib/gateway-api"
import { cn } from "@/lib/utils"

type JsonRecord = Record<string, unknown>
type Operation = "chat" | "responses" | "embeddings"
type PromptRole = "system" | "developer" | "user" | "assistant"
type ChatRole = "user" | "assistant"

type PromptMessage = {
  id: string
  role: PromptRole
  content: string
}

type ConversationMessage = {
  id: string
  role: ChatRole
  content: string
  state?: "streaming" | "stopped" | "error"
  error?: string
  meta?: {
    model?: string
    requestId?: string
    latencyMs?: number
    ttftMs?: number
    inputTokens?: number
    outputTokens?: number
    totalTokens?: number
  }
}

type Variable = { id: string; key: string; value: string }
type Header = { id: string; key: string; value: string }
type Route = {
  id: string
  requested_model: string
  provider?: string
  provider_id?: string
  upstream_model?: string
  priority?: number
  enabled?: boolean
}
type Resource = { id: string; name?: string; provider_type?: string }
type Exchange = {
  request: JsonRecord
  response?: unknown
  headers: Record<string, string>
  timing: {
    totalMs?: number
    ttftMs?: number
    providerLatencyMs?: number
  }
  routing: JsonRecord
}
type EmbeddingResult = {
  vectors: number[][]
  model?: string
  usage?: JsonRecord
  latencyMs: number
  requestId?: string
  raw: unknown
}

const safeDraftKey = "tuenel-playground-safe-draft-v1"

function id() {
  return crypto.randomUUID()
}

function monotonicNow() {
  return performance.now()
}

function pythonLiteral(value: unknown): string {
  if (value === null || value === undefined) return "None"
  if (typeof value === "boolean") return value ? "True" : "False"
  if (typeof value === "number") return String(value)
  if (typeof value === "string") return JSON.stringify(value)
  if (Array.isArray(value))
    return `[${value.map((item) => pythonLiteral(item)).join(", ")}]`
  return `{${Object.entries(value as JsonRecord)
    .map(([key, item]) => `${JSON.stringify(key)}: ${pythonLiteral(item)}`)
    .join(", ")}}`
}

function jsonRecord(value: unknown): JsonRecord {
  return value !== null && typeof value === "object"
    ? (value as JsonRecord)
    : {}
}

function resolveVariables(value: string, variables: Variable[]) {
  const values = new Map(
    variables
      .filter((variable) => variable.key.trim())
      .map((variable) => [variable.key.trim(), variable.value])
  )
  const missing = new Set<string>()
  const resolved = value.replace(
    /{{\s*([\w.-]+)\s*}}/g,
    (match, key: string) => {
      if (!values.has(key)) {
        missing.add(key)
        return match
      }
      return values.get(key) ?? ""
    }
  )
  return { resolved, missing }
}

function errorLabel(error: unknown, timedOut: boolean) {
  if (timedOut) return "Request timed out"
  if (error instanceof GatewayApiError) {
    const code = `${error.code ?? ""} ${error.message}`.toLowerCase()
    if (error.status === 401) return "Invalid API key or expired session"
    if (error.status === 403) return "Request denied by policy"
    if (error.status === 429) return "Rate limit reached"
    if (error.status === 504) return "Request timed out"
    if (error.status === 502 || error.status === 503)
      return "Provider unavailable"
    if (code.includes("unsupported") || code.includes("invalid"))
      return `Unsupported parameter: ${error.message}`
    return error.message
  }
  if (error instanceof SyntaxError) return "Malformed gateway response"
  return error instanceof Error ? error.message : "Request failed"
}

function sanitizeHeaders(headers: Headers) {
  const safe: Record<string, string> = {}
  headers.forEach((value, key) => {
    safe[key] = /(authorization|api-key|token|cookie)/i.test(key)
      ? "[redacted]"
      : value
  })
  return safe
}

function usageFrom(value: unknown) {
  const usage = jsonRecord(value)
  return {
    inputTokens:
      typeof usage.prompt_tokens === "number"
        ? usage.prompt_tokens
        : typeof usage.input_tokens === "number"
          ? usage.input_tokens
          : undefined,
    outputTokens:
      typeof usage.completion_tokens === "number"
        ? usage.completion_tokens
        : typeof usage.output_tokens === "number"
          ? usage.output_tokens
          : undefined,
    totalTokens:
      typeof usage.total_tokens === "number" ? usage.total_tokens : undefined,
  }
}

function Section({
  title,
  children,
  open = false,
}: {
  title: string
  children: React.ReactNode
  open?: boolean
}) {
  const [expanded, setExpanded] = React.useState(open)
  return (
    <details
      open={expanded}
      onToggle={(event) => setExpanded(event.currentTarget.open)}
      className="group border-b last:border-b-0"
    >
      <summary className="flex cursor-pointer list-none items-center justify-between px-3 py-2.5 text-xs font-semibold hover:bg-muted/40">
        {title}
        <span className="text-muted-foreground transition group-open:rotate-90">
          ›
        </span>
      </summary>
      <div className="space-y-3 px-3 pb-4">{children}</div>
    </details>
  )
}

function Label({
  children,
  htmlFor,
}: {
  children: React.ReactNode
  htmlFor?: string
}) {
  return (
    <label
      htmlFor={htmlFor}
      className="block text-[10px] font-medium tracking-wide text-muted-foreground uppercase"
    >
      {children}
    </label>
  )
}

function JsonBlock({ value }: { value: unknown }) {
  return (
    <pre className="max-h-[calc(100dvh-11rem)] overflow-auto rounded-md border bg-muted/30 p-3 text-[11px] whitespace-pre-wrap">
      {JSON.stringify(value ?? {}, null, 2)}
    </pre>
  )
}

export function PlaygroundWorkspace() {
  const { tenantId, projectId } = useGateway()
  const endpoint = useGatewayEndpoint()
  const [operation, setOperation] = React.useState<Operation>("chat")
  const [model, setModel] = React.useState("")
  const [sidebarOpen, setSidebarOpen] = React.useState(true)
  const [inspectorOpen, setInspectorOpen] = React.useState(false)
  const [codeOpen, setCodeOpen] = React.useState(false)
  const [streaming, setStreaming] = React.useState(true)
  const [temperature, setTemperature] = React.useState(0.7)
  const [topP, setTopP] = React.useState(1)
  const [maxTokens, setMaxTokens] = React.useState(1024)
  const [stop, setStop] = React.useState("")
  const [dimensions, setDimensions] = React.useState("")
  const [timeoutMs, setTimeoutMs] = React.useState(60000)
  const [rawOverrides, setRawOverrides] = React.useState("")
  const [promptMessages, setPromptMessages] = React.useState<PromptMessage[]>([
    { id: id(), role: "system", content: "" },
  ])
  const [variables, setVariables] = React.useState<Variable[]>([])
  const [headers, setHeaders] = React.useState<Header[]>([])
  const [conversation, setConversation] = React.useState<ConversationMessage[]>(
    []
  )
  const [composer, setComposer] = React.useState("")
  const [embeddingInput, setEmbeddingInput] = React.useState("")
  const [embeddingBatch, setEmbeddingBatch] = React.useState(false)
  const [embeddingResult, setEmbeddingResult] =
    React.useState<EmbeddingResult>()
  const [running, setRunning] = React.useState(false)
  const [exchange, setExchange] = React.useState<Exchange>()
  const controller = React.useRef<AbortController>(null)
  const skipFirstDraftSave = React.useRef(true)
  const composerRef = React.useRef<HTMLTextAreaElement>(null)

  const adjustComposerHeight = React.useCallback(() => {
    const el = composerRef.current
    if (!el) return
    el.style.height = "auto"
    const newHeight = Math.min(el.scrollHeight, 140)
    el.style.height = `${newHeight}px`
  }, [])

  React.useEffect(() => {
    adjustComposerHeight()
  }, [composer, adjustComposerHeight])

  const models = useGatewayData<{ data: { id: string }[] }>(
    "/v1/models",
    projectId
  )
  const scope = `tenant_id=${encodeURIComponent(tenantId)}&project_id=${encodeURIComponent(projectId ?? "")}`
  const routes = useGatewayData<Page<Route>>(`/admin/model-routes?${scope}`)
  const providers = useGatewayData<Page<Resource>>(
    `/admin/providers?tenant_id=${encodeURIComponent(tenantId)}`
  )
  const policies = useGatewayData<Page<JsonRecord>>(`/admin/policies?${scope}`)
  const selectedModel = model || models.data?.data[0]?.id || ""
  const selectedRoutes = (routes.data?.data ?? [])
    .filter((route) => route.requested_model === selectedModel)
    .sort((left, right) => (left.priority ?? 0) - (right.priority ?? 0))
  const providerNames = new Map(
    (providers.data?.data ?? []).map((provider) => [
      provider.id,
      provider.name ?? provider.id,
    ])
  )
  const primaryRoute = selectedRoutes.find((route) => route.enabled !== false)

  const resolvedPrompts = promptMessages.map((message) => ({
    ...message,
    ...resolveVariables(message.content, variables),
  }))
  const missingVariables = new Set(
    resolvedPrompts.flatMap((message) => [...message.missing])
  )

  React.useEffect(() => {
    if (skipFirstDraftSave.current) {
      skipFirstDraftSave.current = false
      return
    }
    const safeDraft = {
      operation,
      model,
      streaming,
      temperature,
      topP,
      maxTokens,
      dimensions,
      timeoutMs,
    }
    sessionStorage.setItem(safeDraftKey, JSON.stringify(safeDraft))
  }, [
    operation,
    model,
    streaming,
    temperature,
    topP,
    maxTokens,
    dimensions,
    timeoutMs,
  ])

  React.useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape" && controller.current) {
        controller.current.abort()
      }
    }
    window.addEventListener("keydown", onKeyDown)
    return () => window.removeEventListener("keydown", onKeyDown)
  }, [])

  function loadSafeDraft() {
    const saved = sessionStorage.getItem(safeDraftKey)
    if (!saved) return toast.info("No saved settings")
    try {
      const draft = jsonRecord(JSON.parse(saved))
      if (["chat", "responses", "embeddings"].includes(String(draft.operation)))
        setOperation(draft.operation as Operation)
      if (typeof draft.model === "string") setModel(draft.model)
      if (typeof draft.streaming === "boolean") setStreaming(draft.streaming)
      if (typeof draft.temperature === "number")
        setTemperature(draft.temperature)
      if (typeof draft.topP === "number") setTopP(draft.topP)
      if (typeof draft.maxTokens === "number") setMaxTokens(draft.maxTokens)
      if (typeof draft.dimensions === "string") setDimensions(draft.dimensions)
      if (typeof draft.timeoutMs === "number") setTimeoutMs(draft.timeoutMs)
      toast.success("Safe settings restored")
    } catch {
      toast.error("Saved settings are invalid")
    }
  }

  function updatePrompt(id: string, patch: Partial<PromptMessage>) {
    setPromptMessages((current) =>
      current.map((message) =>
        message.id === id ? { ...message, ...patch } : message
      )
    )
  }

  function movePrompt(index: number, direction: -1 | 1) {
    setPromptMessages((current) => {
      const target = index + direction
      if (target < 0 || target >= current.length) return current
      const next = [...current]
      ;[next[index], next[target]] = [next[target], next[index]]
      return next
    })
  }

  function applyOverrides(payload: JsonRecord) {
    if (!rawOverrides.trim()) return payload
    const overrides = JSON.parse(rawOverrides) as unknown
    if (!overrides || typeof overrides !== "object" || Array.isArray(overrides))
      throw new Error("Raw request overrides must be a JSON object")
    const allowed =
      operation === "chat"
        ? new Set([
            "temperature",
            "top_p",
            "max_tokens",
            "max_completion_tokens",
            "stop",
          ])
        : operation === "responses"
          ? new Set(["temperature", "top_p", "max_output_tokens"])
          : new Set(["dimensions"])
    const unsupported = Object.keys(overrides).filter(
      (key) => !allowed.has(key)
    )
    if (unsupported.length)
      throw new Error(`Unsupported parameter: ${unsupported.join(", ")}`)
    return { ...payload, ...(overrides as JsonRecord) }
  }

  function requestHeaders() {
    const value = headers.find(
      (header) => header.key.toLowerCase() === "idempotency-key"
    )?.value
    return {
      "content-type": "application/json",
      ...(value ? { "idempotency-key": value } : {}),
    }
  }

  function routingSnapshot(): JsonRecord {
    return {
      alias: selectedModel,
      configured_targets: selectedRoutes.map((route, index) => ({
        priority: route.priority ?? index,
        role: index === 0 ? "primary" : `fallback_${index}`,
        provider:
          providerNames.get(route.provider ?? route.provider_id ?? "") ??
          route.provider ??
          route.provider_id,
        model: route.upstream_model,
        enabled: route.enabled !== false,
      })),
      chosen_target: "Not returned by the current inference response contract",
      fallback_attempts:
        "Not returned by the current inference response contract",
      retry_decisions:
        "Not returned by the current inference response contract",
      configured_policy_count: policies.data?.data.length ?? 0,
      policy_results: "Not returned by the current inference response contract",
    }
  }

  function buildPayload(
    text: string,
    history: ConversationMessage[]
  ): JsonRecord {
    const fixed = resolvedPrompts
      .filter((message) => message.resolved.trim())
      .map((message) => ({
        role: message.role === "developer" ? "system" : message.role,
        content: message.resolved,
      }))
    const live = history
      .filter((message) => message.content && message.state !== "error")
      .map((message) => ({ role: message.role, content: message.content }))
    const user = { role: "user", content: text }

    if (operation === "chat") {
      return applyOverrides({
        model: selectedModel,
        messages: [...fixed, ...live, user],
        stream: streaming,
        stream_options: streaming ? { include_usage: true } : undefined,
        temperature,
        top_p: topP,
        max_tokens: maxTokens,
        ...(stop.trim() ? { stop: stop.split("\n").filter(Boolean) } : {}),
      })
    }

    const instructions = fixed
      .filter((message) => message.role === "system")
      .map((message) => message.content)
      .join("\n\n")
    return applyOverrides({
      model: selectedModel,
      input: [
        ...fixed.filter((message) => message.role !== "system"),
        ...live,
        user,
      ],
      ...(instructions ? { instructions } : {}),
      stream: streaming,
      temperature,
      top_p: topP,
      max_output_tokens: maxTokens,
    })
  }

  async function send(
    content = composer,
    history = conversation
  ): Promise<void> {
    const text = content.trim()
    if (!text || !selectedModel || running) return
    if (missingVariables.size) {
      toast.error(`Missing variables: ${[...missingVariables].join(", ")}`)
      return
    }

    let payload: JsonRecord
    try {
      payload = buildPayload(text, history)
    } catch (error) {
      toast.error(errorLabel(error, false))
      return
    }

    const abort = new AbortController()
    controller.current = abort
    const started = monotonicNow()
    let firstTokenAt: number | undefined
    let timedOut = false
    const timer = window.setTimeout(() => {
      timedOut = true
      abort.abort()
    }, timeoutMs)
    const userMessage: ConversationMessage = {
      id: id(),
      role: "user",
      content: text,
    }
    const assistantId = id()
    const assistant: ConversationMessage = {
      id: assistantId,
      role: "assistant",
      content: "",
      state: streaming ? "streaming" : undefined,
      meta: { model: selectedModel },
    }
    setConversation([...history, userMessage, assistant])
    setComposer("")
    setRunning(true)
    const rawEvents: unknown[] = []
    let usage: ReturnType<typeof usageFrom> = {
      inputTokens: undefined,
      outputTokens: undefined,
      totalTokens: undefined,
    }
    let requestId: string | undefined

    try {
      const path =
        operation === "responses" ? "/v1/responses" : "/v1/chat/completions"
      const response = await gatewayResponse(
        path,
        tenantId,
        {
          method: "POST",
          headers: {
            ...requestHeaders(),
            ...(streaming ? { accept: "text/event-stream" } : {}),
          },
          body: JSON.stringify(payload),
          signal: abort.signal,
        },
        projectId
      )
      requestId = response.headers.get("x-request-id") ?? undefined

      if (streaming) {
        await readSse(
          response,
          (event) => {
            rawEvents.push(event)
            const value = jsonRecord(event)
            if (value.error) {
              const body = jsonRecord(value.error)
              throw new Error(String(body.message ?? "Streaming interrupted"))
            }
            const choices = Array.isArray(value.choices)
              ? (value.choices as JsonRecord[])
              : []
            const delta =
              String(jsonRecord(choices[0]?.delta).content ?? "") ||
              (value.type === "response.output_text.delta"
                ? String(value.delta ?? "")
                : "")
            if (delta) {
              firstTokenAt ??= monotonicNow()
              setConversation((current) =>
                current.map((message) =>
                  message.id === assistantId
                    ? { ...message, content: message.content + delta }
                    : message
                )
              )
            }
            if (value.usage) usage = usageFrom(value.usage)
          },
          abort.signal
        )
        if (abort.signal.aborted)
          throw new DOMException("Aborted", "AbortError")
      } else {
        const raw = (await response.json()) as unknown
        rawEvents.push(raw)
        const value = jsonRecord(raw)
        const choices = Array.isArray(value.choices)
          ? (value.choices as JsonRecord[])
          : []
        const output = Array.isArray(value.output)
          ? (value.output as JsonRecord[])
          : []
        const outputContent = Array.isArray(output[0]?.content)
          ? (output[0].content as JsonRecord[])
          : []
        const textOutput =
          jsonRecord(choices[0]?.message).content ?? outputContent[0]?.text
        if (typeof textOutput !== "string")
          throw new SyntaxError("Response contains no assistant text")
        firstTokenAt = monotonicNow()
        usage = usageFrom(value.usage)
        setConversation((current) =>
          current.map((message) =>
            message.id === assistantId
              ? { ...message, content: textOutput }
              : message
          )
        )
      }

      const total = monotonicNow() - started
      setConversation((current) =>
        current.map((message) =>
          message.id === assistantId
            ? {
                ...message,
                state: undefined,
                meta: {
                  model: selectedModel,
                  requestId,
                  latencyMs: Math.round(total),
                  ttftMs: firstTokenAt
                    ? Math.round(firstTokenAt - started)
                    : undefined,
                  ...usage,
                },
              }
            : message
        )
      )
      setExchange({
        request: payload,
        response: streaming ? rawEvents : rawEvents[0],
        headers: sanitizeHeaders(response.headers),
        timing: {
          totalMs: Math.round(total),
          ttftMs: firstTokenAt ? Math.round(firstTokenAt - started) : undefined,
        },
        routing: routingSnapshot(),
      })
    } catch (error) {
      const stopped = abort.signal.aborted && !timedOut
      const label = stopped ? "Generation stopped" : errorLabel(error, timedOut)
      setComposer(text)
      setConversation((current) =>
        current.map((message) =>
          message.id === assistantId
            ? {
                ...message,
                state: stopped ? "stopped" : "error",
                error: label,
                meta: { ...message.meta, requestId },
              }
            : message
        )
      )
      if (!stopped) toast.error(label)
    } finally {
      window.clearTimeout(timer)
      controller.current = null
      setRunning(false)
    }
  }

  async function runEmbeddings() {
    const values = embeddingBatch
      ? embeddingInput
          .split("\n")
          .map((value) => value.trim())
          .filter(Boolean)
      : [embeddingInput.trim()]
    if (!values[0] || !selectedModel || running) return
    let payload: JsonRecord
    try {
      payload = applyOverrides({
        model: selectedModel,
        input: embeddingBatch ? values : values[0],
        ...(dimensions ? { dimensions: Number(dimensions) } : {}),
      })
    } catch (error) {
      toast.error(errorLabel(error, false))
      return
    }
    const abort = new AbortController()
    controller.current = abort
    setRunning(true)
    const started = monotonicNow()
    let timedOut = false
    const timer = window.setTimeout(() => {
      timedOut = true
      abort.abort()
    }, timeoutMs)
    try {
      const response = await gatewayResponse(
        "/v1/embeddings",
        tenantId,
        {
          method: "POST",
          headers: requestHeaders(),
          body: JSON.stringify(payload),
          signal: abort.signal,
        },
        projectId
      )
      const raw = (await response.json()) as unknown
      const value = jsonRecord(raw)
      if (!Array.isArray(value.data))
        throw new SyntaxError("Response contains no embedding data")
      const vectors = (value.data as JsonRecord[]).map((item) =>
        Array.isArray(item.embedding) ? (item.embedding as number[]) : []
      )
      const latencyMs = Math.round(monotonicNow() - started)
      const requestId = response.headers.get("x-request-id") ?? undefined
      setEmbeddingResult({
        vectors,
        model: typeof value.model === "string" ? value.model : undefined,
        usage: value.usage ? jsonRecord(value.usage) : undefined,
        latencyMs,
        requestId,
        raw,
      })
      setExchange({
        request: payload,
        response: raw,
        headers: sanitizeHeaders(response.headers),
        timing: { totalMs: latencyMs },
        routing: routingSnapshot(),
      })
    } catch (error) {
      if (!abort.signal.aborted || timedOut)
        toast.error(errorLabel(error, timedOut))
    } finally {
      window.clearTimeout(timer)
      controller.current = null
      setRunning(false)
    }
  }

  function editMessage(messageId: string) {
    const index = conversation.findIndex((message) => message.id === messageId)
    if (index < 0) return
    setComposer(conversation[index].content)
    setConversation(conversation.slice(0, index))
  }

  function regenerate() {
    const index = conversation.findLastIndex(
      (message) => message.role === "user"
    )
    if (index < 0) return
    void send(conversation[index].content, conversation.slice(0, index))
  }

  const codeFixedMessages = resolvedPrompts
    .filter((message) => message.resolved.trim())
    .map((message) => ({
      role: message.role === "developer" ? "system" : message.role,
      content: message.resolved,
    }))
  const codeInstructions = codeFixedMessages
    .filter((message) => message.role === "system")
    .map((message) => message.content)
    .join("\n\n")
  const codePayload: JsonRecord =
    operation === "embeddings"
      ? {
          model: selectedModel || "MODEL_ALIAS",
          input: embeddingBatch
            ? embeddingInput.trim()
              ? embeddingInput
                  .split("\n")
                  .map((value) => value.trim())
                  .filter(Boolean)
              : ["First text", "Second text"]
            : embeddingInput || "Your text",
          ...(dimensions ? { dimensions: Number(dimensions) } : {}),
        }
      : operation === "chat"
        ? {
            model: selectedModel || "MODEL_ALIAS",
            messages: [
              ...codeFixedMessages,
              { role: "user", content: composer || "Hello" },
            ],
            stream: streaming,
            ...(streaming ? { stream_options: { include_usage: true } } : {}),
            temperature,
            top_p: topP,
            max_tokens: maxTokens,
            ...(stop.trim() ? { stop: stop.split("\n").filter(Boolean) } : {}),
          }
        : {
            model: selectedModel || "MODEL_ALIAS",
            input: [
              ...codeFixedMessages.filter(
                (message) => message.role !== "system"
              ),
              { role: "user", content: composer || "Hello" },
            ],
            ...(codeInstructions ? { instructions: codeInstructions } : {}),
            stream: streaming,
            temperature,
            top_p: topP,
            max_output_tokens: maxTokens,
          }
  let previewPayload = codePayload
  try {
    previewPayload = applyOverrides(codePayload)
  } catch {
    // The editor shows validation; code stays runnable with supported controls.
  }
  const operationPath =
    operation === "chat"
      ? "chat/completions"
      : operation === "responses"
        ? "responses"
        : "embeddings"
  const codeExamples = {
    curl: `curl "${endpoint}/${operationPath}" \\\n  -H "Authorization: Bearer $TUNEL_API_KEY" \\\n  -H "Content-Type: application/json" \\\n  -d '${JSON.stringify(previewPayload)}'`,
    python: `import os\nfrom openai import OpenAI\n\nclient = OpenAI(\n    api_key=os.environ["TUNEL_API_KEY"],\n    base_url="${endpoint}",\n)\n\nresult = client.${operation === "chat" ? "chat.completions" : operation}.create(**${pythonLiteral(previewPayload)})`,
    javascript: `import OpenAI from "openai";\n\nconst client = new OpenAI({\n  apiKey: process.env.TUNEL_API_KEY,\n  baseURL: "${endpoint}",\n});\n\nconst result = await client.${operation === "chat" ? "chat.completions" : operation}.create(${JSON.stringify(previewPayload, null, 2)});`,
    http: `const response = await fetch("${endpoint}/${operationPath}", {\n  method: "POST",\n  headers: {\n    Authorization: \`Bearer \${process.env.TUNEL_API_KEY}\`,\n    "Content-Type": "application/json",\n  },\n  body: JSON.stringify(${JSON.stringify(previewPayload, null, 2)}),\n});`,
  }

  return (
    <div className="flex h-full min-h-0 flex-1 flex-col overflow-hidden rounded-lg border bg-background">
      <header className="flex h-12 shrink-0 items-center justify-between border-b px-3">
        <div className="flex min-w-0 items-center gap-2">
          <Button
            size="icon-sm"
            variant="ghost"
            onClick={() => setSidebarOpen((value) => !value)}
            aria-label="Toggle configuration"
          >
            <SidebarSimpleIcon />
          </Button>
          <div>
            <h1 className="text-sm font-semibold">Playground</h1>
            <p className="hidden text-[10px] text-muted-foreground sm:block">
              Test real project aliases through Tuenel
            </p>
          </div>
        </div>
        <div className="flex items-center gap-1.5">
          <NativeSelect
            size="sm"
            value={operation}
            onChange={(event) => setOperation(event.target.value as Operation)}
            aria-label="Operation"
          >
            <NativeSelectOption value="chat">Chat</NativeSelectOption>
            <NativeSelectOption value="responses">Responses</NativeSelectOption>
            <NativeSelectOption value="embeddings">
              Embeddings
            </NativeSelectOption>
          </NativeSelect>
          <Button size="sm" variant="outline" onClick={() => setCodeOpen(true)}>
            <CodeIcon data-icon="inline-start" />
            View code
          </Button>
          <Button
            size="sm"
            variant="outline"
            disabled={!exchange}
            onClick={() => setInspectorOpen(true)}
          >
            Inspector
          </Button>
          <Button
            size="icon-sm"
            variant="ghost"
            onClick={() => {
              setConversation([])
              setEmbeddingResult(undefined)
              setExchange(undefined)
            }}
            aria-label="New conversation"
          >
            <TrashIcon />
          </Button>
        </div>
      </header>

      <div className="relative flex min-h-0 flex-1">
        {sidebarOpen && (
          <aside className="absolute inset-y-0 left-0 z-20 w-[min(350px,92vw)] shrink-0 overflow-y-auto border-r bg-background shadow-xl lg:static lg:w-[350px] lg:bg-muted/10 lg:shadow-none">
            <div className="sticky top-0 z-10 flex h-10 items-center border-b bg-background/95 px-3 text-xs font-semibold backdrop-blur lg:bg-muted/95">
              Configuration
            </div>
            <Section title="Prompt" open>
              <p className="text-[10px] text-muted-foreground">
                Fixed prompt configuration. It stays when you start a new
                conversation.
              </p>
              {promptMessages.map((message, index) => (
                <div key={message.id} className="rounded-md border bg-card p-2">
                  <div className="mb-2 flex items-center gap-1">
                    <NativeSelect
                      size="sm"
                      value={message.role}
                      onChange={(event) =>
                        updatePrompt(message.id, {
                          role: event.target.value as PromptRole,
                        })
                      }
                    >
                      {["system", "developer", "user", "assistant"].map(
                        (role) => (
                          <NativeSelectOption key={role} value={role}>
                            {role}
                          </NativeSelectOption>
                        )
                      )}
                    </NativeSelect>
                    <div className="ml-auto flex">
                      <Button
                        size="icon-xs"
                        variant="ghost"
                        disabled={index === 0}
                        onClick={() => movePrompt(index, -1)}
                        aria-label="Move up"
                      >
                        <ArrowUpIcon />
                      </Button>
                      <Button
                        size="icon-xs"
                        variant="ghost"
                        disabled={index === promptMessages.length - 1}
                        onClick={() => movePrompt(index, 1)}
                        aria-label="Move down"
                      >
                        <ArrowDownIcon />
                      </Button>
                      <Button
                        size="icon-xs"
                        variant="ghost"
                        onClick={() =>
                          setPromptMessages((current) => [
                            ...current.slice(0, index + 1),
                            { ...message, id: id() },
                            ...current.slice(index + 1),
                          ])
                        }
                        aria-label="Duplicate"
                      >
                        <CopyIcon />
                      </Button>
                      <Button
                        size="icon-xs"
                        variant="ghost"
                        onClick={() =>
                          setPromptMessages((current) =>
                            current.filter((item) => item.id !== message.id)
                          )
                        }
                        aria-label="Delete"
                      >
                        <TrashIcon />
                      </Button>
                    </div>
                  </div>
                  <Textarea
                    rows={3}
                    value={message.content}
                    placeholder={`Fixed ${message.role} message`}
                    onChange={(event) =>
                      updatePrompt(message.id, {
                        content: event.target.value,
                      })
                    }
                  />
                  {message.role === "developer" && (
                    <p className="mt-1 text-[9px] text-muted-foreground">
                      Sent as a system instruction by the current gateway
                      contract.
                    </p>
                  )}
                </div>
              ))}
              <Button
                size="sm"
                variant="outline"
                className="w-full"
                onClick={() =>
                  setPromptMessages((current) => [
                    ...current,
                    { id: id(), role: "user", content: "" },
                  ])
                }
              >
                <PlusIcon data-icon="inline-start" />
                Add message
              </Button>
            </Section>

            <Section title="Model" open>
              <div className="space-y-1">
                <Label htmlFor="playground-model">Model alias</Label>
                <NativeSelect
                  id="playground-model"
                  className="w-full"
                  value={selectedModel}
                  disabled={models.loading || !models.data?.data.length}
                  onChange={(event) => setModel(event.target.value)}
                >
                  {!selectedModel && (
                    <NativeSelectOption value="">
                      No project aliases
                    </NativeSelectOption>
                  )}
                  {models.data?.data.map((item) => (
                    <NativeSelectOption key={item.id} value={item.id}>
                      {item.id}
                    </NativeSelectOption>
                  ))}
                </NativeSelect>
                {models.error && (
                  <p className="text-[10px] text-destructive">
                    Could not load project aliases.
                  </p>
                )}
              </div>
              {primaryRoute && (
                <div className="rounded-md border bg-muted/30 p-2 text-[10px]">
                  <span className="text-muted-foreground">
                    Configured primary
                  </span>
                  <p className="mt-0.5 font-mono">
                    {providerNames.get(
                      primaryRoute.provider ?? primaryRoute.provider_id ?? ""
                    ) ??
                      primaryRoute.provider ??
                      primaryRoute.provider_id}{" "}
                    / {primaryRoute.upstream_model}
                  </p>
                </div>
              )}
              {operation !== "embeddings" && (
                <>
                  <div className="space-y-1">
                    <Label>Response format</Label>
                    <NativeSelect value="text" disabled className="w-full">
                      <NativeSelectOption value="text">Text</NativeSelectOption>
                    </NativeSelect>
                    <p className="text-[9px] text-muted-foreground">
                      JSON object and schema are hidden until the gateway
                      contract supports them.
                    </p>
                  </div>
                  <div className="flex items-center justify-between">
                    <Label>Stream response</Label>
                    <Switch
                      size="sm"
                      checked={streaming}
                      onCheckedChange={setStreaming}
                    />
                  </div>
                </>
              )}
            </Section>

            <Section title="Parameters" open>
              {operation === "embeddings" ? (
                <div className="space-y-1">
                  <Label htmlFor="embedding-dimensions">
                    Dimensions (optional)
                  </Label>
                  <Input
                    id="embedding-dimensions"
                    type="number"
                    min={1}
                    max={32768}
                    value={dimensions}
                    onChange={(event) => setDimensions(event.target.value)}
                  />
                </div>
              ) : (
                <>
                  <Parameter
                    label="Temperature"
                    value={temperature}
                    min={0}
                    max={2}
                    step={0.1}
                    onChange={setTemperature}
                  />
                  <Parameter
                    label="Top P"
                    value={topP}
                    min={0}
                    max={1}
                    step={0.05}
                    onChange={setTopP}
                  />
                  <div className="space-y-1">
                    <Label htmlFor="max-output-tokens">Max output tokens</Label>
                    <Input
                      id="max-output-tokens"
                      type="number"
                      min={1}
                      value={maxTokens}
                      onChange={(event) =>
                        setMaxTokens(Number(event.target.value))
                      }
                    />
                  </div>
                  {operation === "chat" && (
                    <div className="space-y-1">
                      <Label htmlFor="stop-sequences">Stop sequences</Label>
                      <Textarea
                        id="stop-sequences"
                        rows={2}
                        value={stop}
                        placeholder="One per line"
                        onChange={(event) => setStop(event.target.value)}
                      />
                    </div>
                  )}
                </>
              )}
              <p className="text-[9px] text-muted-foreground">
                Unsupported seed, penalties, and reasoning controls are not
                sent.
              </p>
              <Button
                size="sm"
                variant="ghost"
                onClick={() => {
                  setTemperature(0.7)
                  setTopP(1)
                  setMaxTokens(1024)
                  setStop("")
                  setDimensions("")
                }}
              >
                <ArrowCounterClockwiseIcon data-icon="inline-start" />
                Reset defaults
              </Button>
            </Section>

            <Section title="Variables">
              {variables.map((variable) => (
                <div key={variable.id} className="grid grid-cols-2 gap-2">
                  <Input
                    value={variable.key}
                    placeholder="customer_name"
                    onChange={(event) =>
                      setVariables((current) =>
                        current.map((item) =>
                          item.id === variable.id
                            ? { ...item, key: event.target.value }
                            : item
                        )
                      )
                    }
                  />
                  <div className="flex gap-1">
                    <Input
                      value={variable.value}
                      placeholder="Ada"
                      onChange={(event) =>
                        setVariables((current) =>
                          current.map((item) =>
                            item.id === variable.id
                              ? { ...item, value: event.target.value }
                              : item
                          )
                        )
                      }
                    />
                    <Button
                      size="icon-sm"
                      variant="ghost"
                      onClick={() =>
                        setVariables((current) =>
                          current.filter((item) => item.id !== variable.id)
                        )
                      }
                      aria-label="Delete variable"
                    >
                      <TrashIcon />
                    </Button>
                  </div>
                </div>
              ))}
              <Button
                size="sm"
                variant="outline"
                onClick={() =>
                  setVariables((current) => [
                    ...current,
                    { id: id(), key: "", value: "" },
                  ])
                }
              >
                <PlusIcon data-icon="inline-start" />
                Add variable
              </Button>
              {!!missingVariables.size && (
                <p className="text-[10px] text-destructive">
                  Missing: {[...missingVariables].join(", ")}
                </p>
              )}
              <div>
                <Label>Resolved prompt preview</Label>
                <pre className="mt-1 max-h-32 overflow-auto rounded-md border bg-muted/30 p-2 text-[10px] whitespace-pre-wrap">
                  {resolvedPrompts
                    .filter((message) => message.resolved)
                    .map((message) => `${message.role}: ${message.resolved}`)
                    .join("\n\n") || "No fixed prompt messages."}
                </pre>
              </div>
            </Section>

            <Section title="Tools">
              {["Hosted tools", "Local tools"].map((group) => (
                <div key={group}>
                  <Label>{group}</Label>
                  <div className="mt-1 grid grid-cols-2 gap-1.5">
                    {(group === "Hosted tools"
                      ? [
                          "MCP servers",
                          "Function tools",
                          "Custom tools",
                          "Web search",
                          "File search",
                          "Image generation",
                          "Code interpreter",
                        ]
                      : ["Local shell", "Hosted shell"]
                    ).map((tool) => (
                      <button
                        key={tool}
                        disabled
                        className="rounded-md border p-2 text-left opacity-55"
                      >
                        <WrenchIcon className="mb-1 size-3" />
                        <span className="block text-[10px]">{tool}</span>
                        <span className="text-[8px] text-muted-foreground">
                          Coming soon
                        </span>
                      </button>
                    ))}
                  </div>
                </div>
              ))}
            </Section>

            <Section title="Advanced">
              <div>
                <div className="flex items-center justify-between">
                  <Label>Custom request headers</Label>
                  <Button
                    size="xs"
                    variant="ghost"
                    onClick={() =>
                      setHeaders((current) => [
                        ...current,
                        { id: id(), key: "", value: "" },
                      ])
                    }
                  >
                    Add
                  </Button>
                </div>
                {headers.map((header) => (
                  <div key={header.id} className="mt-1 grid grid-cols-2 gap-1">
                    <Input
                      value={header.key}
                      placeholder="Idempotency-Key"
                      onChange={(event) =>
                        setHeaders((current) =>
                          current.map((item) =>
                            item.id === header.id
                              ? { ...item, key: event.target.value }
                              : item
                          )
                        )
                      }
                    />
                    <Input
                      value={header.value}
                      placeholder="value"
                      onChange={(event) =>
                        setHeaders((current) =>
                          current.map((item) =>
                            item.id === header.id
                              ? { ...item, value: event.target.value }
                              : item
                          )
                        )
                      }
                    />
                    {header.key &&
                      header.key.toLowerCase() !== "idempotency-key" && (
                        <p className="col-span-2 text-[9px] text-destructive">
                          The browser gateway only forwards Idempotency-Key.
                        </p>
                      )}
                  </div>
                ))}
              </div>
              {["Metadata", "User identifier"].map((label) => (
                <div key={label} className="space-y-1">
                  <Label>{label}</Label>
                  <Input
                    disabled
                    placeholder="Not supported by the current gateway contract"
                  />
                </div>
              ))}
              <div className="space-y-1">
                <Label htmlFor="request-timeout">Timeout (ms)</Label>
                <Input
                  id="request-timeout"
                  type="number"
                  min={1000}
                  value={timeoutMs}
                  onChange={(event) => setTimeoutMs(Number(event.target.value))}
                />
              </div>
              <div className="space-y-1">
                <Label htmlFor="raw-overrides">Raw request overrides</Label>
                <Textarea
                  id="raw-overrides"
                  rows={4}
                  value={rawOverrides}
                  placeholder='{"temperature": 0.2}'
                  onChange={(event) => setRawOverrides(event.target.value)}
                />
                <p className="text-[9px] text-muted-foreground">
                  Unsupported keys are rejected before sending.
                </p>
              </div>
              <Button size="sm" variant="outline" onClick={loadSafeDraft}>
                Load saved safe settings
              </Button>
              <p className="text-[9px] text-muted-foreground">
                Prompts, conversation, headers, raw JSON, and credentials are
                never stored.
              </p>
            </Section>
          </aside>
        )}

        <main className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden bg-background">
          {operation === "embeddings" ? (
            <EmbeddingsCanvas
              input={embeddingInput}
              setInput={setEmbeddingInput}
              batch={embeddingBatch}
              setBatch={setEmbeddingBatch}
              result={embeddingResult}
              running={running}
              disabled={!selectedModel}
              onRun={() => void runEmbeddings()}
              onStop={() => controller.current?.abort()}
              onInspect={() => setInspectorOpen(true)}
            />
          ) : (
            <>
              <div className="min-h-0 flex-1 overflow-y-auto overscroll-contain">
                <div className="mx-auto flex min-h-full max-w-4xl flex-col px-4 pt-5 pb-10 sm:px-8">
                  {!conversation.length ? (
                    <div className="my-auto py-16 text-center">
                      <div className="mx-auto mb-4 flex size-10 items-center justify-center rounded-xl border bg-muted/30 font-semibold">
                        T
                      </div>
                      <h2 className="text-base font-semibold">
                        Start a conversation
                      </h2>
                      <p className="mx-auto mt-1 max-w-md text-xs text-muted-foreground">
                        Select a real model alias, tune only supported
                        parameters, then send a message.
                      </p>
                      {!models.loading && !models.data?.data.length && (
                        <Alert className="mx-auto mt-5 max-w-md text-left">
                          <AlertTitle>No model aliases available</AlertTitle>
                          <AlertDescription>
                            Configure a model route before using Playground.
                          </AlertDescription>
                        </Alert>
                      )}
                    </div>
                  ) : (
                    <div className="space-y-6">
                      {conversation.map((message) => (
                        <MessageBlock
                          key={message.id}
                          message={message}
                          onEdit={() => editMessage(message.id)}
                        />
                      ))}
                    </div>
                  )}
                </div>
              </div>
              <div className="sticky bottom-0 z-10 shrink-0 border-t bg-background/95 p-3 backdrop-blur">
                <div className="mx-auto max-w-4xl">
                  <div className="flex flex-col rounded-xl border bg-card shadow-sm focus-within:ring-2 focus-within:ring-ring/30">
                    <Textarea
                      ref={composerRef}
                      className="min-h-[44px] max-h-[140px] w-full resize-none overflow-y-auto border-0 bg-transparent px-3 py-2.5 text-sm leading-relaxed shadow-none focus-visible:ring-0 focus-visible:ring-offset-0 disabled:cursor-not-allowed disabled:opacity-50"
                      value={composer}
                      placeholder={
                        selectedModel
                          ? `Message ${selectedModel}`
                          : "Configure a model alias to begin"
                      }
                      disabled={!selectedModel}
                      onChange={(event) => setComposer(event.target.value)}
                      onKeyDown={(event) => {
                        if (event.key === "Enter" && !event.shiftKey) {
                          event.preventDefault()
                          void send()
                        }
                      }}
                    />
                    <div className="flex shrink-0 items-center justify-between px-3 pb-2 pt-1">
                      <p className="text-[10px] text-muted-foreground">
                        Enter to send · Shift + Enter for newline · Esc to stop
                      </p>
                      <div className="flex gap-1">
                        {!!conversation.length && !running && (
                          <Button
                            size="sm"
                            variant="ghost"
                            onClick={regenerate}
                          >
                            <ArrowCounterClockwiseIcon data-icon="inline-start" />
                            Regenerate
                          </Button>
                        )}
                        {running ? (
                          <Button
                            size="icon-sm"
                            variant="outline"
                            onClick={() => controller.current?.abort()}
                            aria-label="Stop generation"
                          >
                            <StopIcon weight="fill" />
                          </Button>
                        ) : (
                          <Button
                            size="icon-sm"
                            disabled={!composer.trim() || !selectedModel}
                            onClick={() => void send()}
                            aria-label="Send message"
                          >
                            <PaperPlaneTiltIcon weight="fill" />
                          </Button>
                        )}
                      </div>
                    </div>
                  </div>
                </div>
              </div>
            </>
          )}
        </main>
      </div>

      <Inspector
        open={inspectorOpen}
        onOpenChange={setInspectorOpen}
        exchange={exchange}
      />
      <CodeSheet
        open={codeOpen}
        onOpenChange={setCodeOpen}
        examples={codeExamples}
      />
    </div>
  )
}

function Parameter({
  label,
  value,
  min,
  max,
  step,
  onChange,
}: {
  label: string
  value: number
  min: number
  max: number
  step: number
  onChange: (value: number) => void
}) {
  return (
    <div className="space-y-1">
      <div className="flex items-center justify-between">
        <Label>{label}</Label>
        <Input
          className="h-6 w-16 px-1 text-right text-[10px]"
          type="number"
          min={min}
          max={max}
          step={step}
          value={value}
          onChange={(event) => onChange(Number(event.target.value))}
        />
      </div>
      <input
        className="h-1 w-full accent-primary"
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(event) => onChange(Number(event.target.value))}
      />
    </div>
  )
}

function MessageBlock({
  message,
  onEdit,
}: {
  message: ConversationMessage
  onEdit: () => void
}) {
  const assistant = message.role === "assistant"
  return (
    <article
      className={cn(
        "group flex gap-3",
        !assistant && "ml-auto max-w-[85%] flex-row-reverse"
      )}
    >
      <div
        className={cn(
          "mt-0.5 flex size-7 shrink-0 items-center justify-center rounded-md border text-[10px] font-semibold",
          assistant ? "bg-muted/40" : "bg-primary text-primary-foreground"
        )}
      >
        {assistant ? "T" : "You"}
      </div>
      <div className="min-w-0 flex-1">
        {assistant ? (
          <>
            <MarkdownContent content={message.content} />
            {message.state === "streaming" && (
              <div className="text-sm leading-6">
                <span className="ml-1 inline-block size-1.5 animate-pulse rounded-full bg-primary" />
                {!message.content && (
                  <span className="inline-flex items-center gap-2 text-muted-foreground">
                    <Spinner className="size-3" /> Generating
                  </span>
                )}
              </div>
            )}
            {message.error && (
              <p
                className={cn(
                  "mt-2 text-xs",
                  message.state === "error"
                    ? "text-destructive"
                    : "text-muted-foreground"
                )}
              >
                {message.error}
              </p>
            )}
            {message.meta && (
              <div className="mt-2 flex flex-wrap gap-x-3 gap-y-1 text-[9px] text-muted-foreground">
                {message.meta.model && <span>{message.meta.model}</span>}
                {message.meta.latencyMs !== undefined && (
                  <span>{message.meta.latencyMs} ms</span>
                )}
                {message.meta.ttftMs !== undefined && (
                  <span>TTFT {message.meta.ttftMs} ms</span>
                )}
                {message.meta.inputTokens !== undefined && (
                  <span>{message.meta.inputTokens} in</span>
                )}
                {message.meta.outputTokens !== undefined && (
                  <span>{message.meta.outputTokens} out</span>
                )}
                {message.meta.requestId && (
                  <span className="font-mono">ID {message.meta.requestId}</span>
                )}
              </div>
            )}
            <div className="mt-1 flex h-6 items-center gap-1 opacity-0 transition group-hover:opacity-100 focus-within:opacity-100">
              <Button
                size="icon-xs"
                variant="ghost"
                onClick={() =>
                  navigator.clipboard
                    .writeText(message.content)
                    .then(() => toast.success("Response copied"))
                }
                aria-label="Copy response"
              >
                <CopyIcon />
              </Button>
            </div>
          </>
        ) : (
          <div className="flex flex-col items-end">
            <div className="rounded-xl bg-muted px-4 py-2.5 text-sm leading-6 whitespace-pre-wrap">
              {message.content}
            </div>
            <div className="mt-1 flex h-6 items-center justify-end opacity-0 transition group-hover:opacity-100 focus-within:opacity-100">
              <Button
                size="icon-xs"
                variant="ghost"
                onClick={onEdit}
                aria-label="Edit message"
              >
                <PencilSimpleIcon />
              </Button>
            </div>
          </div>
        )}
      </div>
    </article>
  )
}

function renderInlineMarkdown(value: string): React.ReactNode[] {
  const token =
    /(`[^`]+`|\*\*[^*]+\*\*|__[^_]+__|\*[^*]+\*|_[^_]+_|\[[^\]]+\]\([^\s)]+\))/g
  const parts: React.ReactNode[] = []
  let cursor = 0
  let match: RegExpExecArray | null
  let key = 0
  while ((match = token.exec(value))) {
    if (match.index > cursor) parts.push(value.slice(cursor, match.index))
    const raw = match[0]
    if (raw.startsWith("`") && raw.endsWith("`")) {
      parts.push(
        <code
          key={key++}
          className="rounded bg-muted px-1 py-0.5 font-mono text-[0.9em]"
        >
          {raw.slice(1, -1)}
        </code>
      )
    } else if (raw.startsWith("**") || raw.startsWith("__")) {
      parts.push(<strong key={key++}>{raw.slice(2, -2)}</strong>)
    } else if (raw.startsWith("*") || raw.startsWith("_")) {
      parts.push(<em key={key++}>{raw.slice(1, -1)}</em>)
    } else {
      const link = /^\[([^\]]+)\]\(([^\s)]+)\)$/.exec(raw)
      if (link)
        parts.push(
          <a
            key={key++}
            href={link[2]}
            target="_blank"
            rel="noreferrer"
            className="text-primary underline underline-offset-2"
          >
            {link[1]}
          </a>
        )
      else parts.push(raw)
    }
    cursor = match.index + raw.length
  }
  if (cursor < value.length) parts.push(value.slice(cursor))
  return parts
}

function MarkdownContent({ content }: { content: string }) {
  const lines = content.split(/\r?\n/)
  const blocks: React.ReactNode[] = []
  let index = 0
  let key = 0
  while (index < lines.length) {
    const line = lines[index]
    if (!line.trim()) {
      index += 1
      continue
    }
    if (line.startsWith("```")) {
      const language = line.slice(3).trim()
      const code: string[] = []
      index += 1
      while (index < lines.length && !lines[index].startsWith("```")) {
        code.push(lines[index])
        index += 1
      }
      if (index < lines.length) index += 1
      blocks.push(
        <pre
          key={key++}
          className="my-3 overflow-x-auto rounded-md border bg-muted/50 p-3 text-xs"
        >
          {language && (
            <span className="mb-2 block text-[10px] text-muted-foreground">
              {language}
            </span>
          )}
          <code>{code.join("\n")}</code>
        </pre>
      )
      continue
    }
    const heading = /^(#{1,6})\s+(.+)$/.exec(line)
    if (heading) {
      const level = heading[1].length
      const Heading = `h${level}` as keyof React.JSX.IntrinsicElements
      blocks.push(
        <Heading
          key={key++}
          className={cn(
            "mt-4 font-heading font-semibold first:mt-0",
            level === 1 ? "text-xl" : level === 2 ? "text-lg" : "text-base"
          )}
        >
          {renderInlineMarkdown(heading[2])}
        </Heading>
      )
      index += 1
      continue
    }
    if (/^\s*[-*+]\s+/.test(line) || /^\s*\d+\.\s+/.test(line)) {
      const ordered = /^\s*\d+\.\s+/.test(line)
      const items: string[] = []
      while (index < lines.length) {
        const item = ordered
          ? /^\s*\d+\.\s+(.+)$/.exec(lines[index])
          : /^\s*[-*+]\s+(.+)$/.exec(lines[index])
        if (!item) break
        items.push(item[1])
        index += 1
      }
      const List = ordered ? "ol" : "ul"
      blocks.push(
        <List
          key={key++}
          className={cn(
            "my-2 space-y-1 pl-6 text-sm leading-6",
            ordered ? "list-decimal" : "list-disc"
          )}
        >
          {items.map((item, itemIndex) => (
            <li key={itemIndex}>{renderInlineMarkdown(item)}</li>
          ))}
        </List>
      )
      continue
    }
    const paragraph: string[] = [line]
    index += 1
    while (
      index < lines.length &&
      lines[index].trim() &&
      !/^```|^#{1,6}\s|^\s*[-*+]\s+|^\s*\d+\.\s+/.test(lines[index])
    ) {
      paragraph.push(lines[index])
      index += 1
    }
    blocks.push(
      <p
        key={key++}
        className="my-2 text-sm leading-6 whitespace-pre-wrap first:mt-0 last:mb-0"
      >
        {renderInlineMarkdown(paragraph.join("\n"))}
      </p>
    )
  }
  return <div className="text-sm leading-6">{blocks}</div>
}

function EmbeddingsCanvas({
  input,
  setInput,
  batch,
  setBatch,
  result,
  running,
  disabled,
  onRun,
  onStop,
  onInspect,
}: {
  input: string
  setInput: (value: string) => void
  batch: boolean
  setBatch: (value: boolean) => void
  result?: EmbeddingResult
  running: boolean
  disabled: boolean
  onRun: () => void
  onStop: () => void
  onInspect: () => void
}) {
  return (
    <div className="min-h-0 flex-1 overflow-y-auto p-4 sm:p-8">
      <div className="mx-auto max-w-4xl space-y-4">
        <div>
          <h2 className="text-base font-semibold">Create embeddings</h2>
          <p className="text-xs text-muted-foreground">
            Embed one text or one non-empty line per batch item.
          </p>
        </div>
        <div className="rounded-lg border bg-card p-4">
          <div className="mb-2 flex items-center justify-between">
            <Label htmlFor="embedding-input">Input</Label>
            <label className="flex items-center gap-2 text-[10px]">
              Batch
              <Switch size="sm" checked={batch} onCheckedChange={setBatch} />
            </label>
          </div>
          <Textarea
            id="embedding-input"
            rows={10}
            value={input}
            disabled={disabled}
            placeholder={batch ? "First text\nSecond text" : "Text to embed"}
            onChange={(event) => setInput(event.target.value)}
          />
          <div className="mt-3 flex justify-end">
            {running ? (
              <Button variant="outline" onClick={onStop}>
                <StopIcon data-icon="inline-start" />
                Stop
              </Button>
            ) : (
              <Button disabled={disabled || !input.trim()} onClick={onRun}>
                <PaperPlaneTiltIcon data-icon="inline-start" />
                Run embeddings
              </Button>
            )}
          </div>
        </div>
        {result && (
          <div className="rounded-lg border bg-card p-4">
            <div className="flex items-start justify-between">
              <div>
                <h3 className="text-sm font-semibold">Embedding result</h3>
                <div className="mt-1 flex flex-wrap gap-3 text-[10px] text-muted-foreground">
                  <span>{result.vectors.length} vectors</span>
                  <span>{result.vectors[0]?.length ?? 0} dimensions</span>
                  <span>{result.latencyMs} ms</span>
                  {typeof result.usage?.total_tokens === "number" && (
                    <span>{result.usage.total_tokens} tokens</span>
                  )}
                  {result.requestId && (
                    <span className="font-mono">{result.requestId}</span>
                  )}
                </div>
              </div>
              <Button size="sm" variant="outline" onClick={onInspect}>
                Raw JSON
              </Button>
            </div>
            <div className="mt-4 space-y-2">
              {result.vectors.map((vector, index) => (
                <div
                  key={index}
                  className="rounded-md bg-muted/30 p-2 font-mono text-[10px]"
                >
                  [{vector.slice(0, 8).join(", ")}
                  {vector.length > 8 ? ", …" : ""}]
                </div>
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  )
}

function Inspector({
  open,
  onOpenChange,
  exchange,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  exchange?: Exchange
}) {
  const downloadable = exchange ?? {}
  function copy() {
    void navigator.clipboard
      .writeText(JSON.stringify(downloadable, null, 2))
      .then(() => toast.success("Inspector JSON copied"))
  }
  function download() {
    const url = URL.createObjectURL(
      new Blob([JSON.stringify(downloadable, null, 2)], {
        type: "application/json",
      })
    )
    const link = document.createElement("a")
    link.href = url
    link.download = "tuenel-playground-exchange.json"
    link.click()
    URL.revokeObjectURL(url)
  }
  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent className="w-[min(520px,94vw)] overflow-hidden sm:max-w-[520px]">
        <SheetHeader className="border-b p-4">
          <SheetTitle>Request inspector</SheetTitle>
          <SheetDescription>
            Sanitized data returned by the current gateway exchange.
          </SheetDescription>
          <div className="flex gap-2 pt-2">
            <Button size="sm" variant="outline" onClick={copy}>
              <CopyIcon data-icon="inline-start" />
              Copy JSON
            </Button>
            <Button size="sm" variant="outline" onClick={download}>
              <DownloadSimpleIcon data-icon="inline-start" />
              Download
            </Button>
          </div>
        </SheetHeader>
        <Tabs
          defaultValue="request"
          className="min-h-0 flex-1 overflow-hidden p-4"
        >
          <TabsList className="max-w-full overflow-x-auto">
            {["request", "response", "headers", "timing", "routing"].map(
              (tab) => (
                <TabsTrigger key={tab} value={tab}>
                  {tab === "response" ? "Raw response" : tab}
                </TabsTrigger>
              )
            )}
          </TabsList>
          <TabsContent value="request" className="min-h-0 overflow-y-auto">
            <JsonBlock value={exchange?.request} />
          </TabsContent>
          <TabsContent value="response" className="min-h-0 overflow-y-auto">
            <JsonBlock value={exchange?.response} />
          </TabsContent>
          <TabsContent value="headers" className="min-h-0 overflow-y-auto">
            <JsonBlock value={exchange?.headers} />
          </TabsContent>
          <TabsContent value="timing" className="min-h-0 overflow-y-auto">
            <JsonBlock
              value={{
                time_to_first_token_ms: exchange?.timing.ttftMs,
                total_duration_ms: exchange?.timing.totalMs,
                provider_latency_ms:
                  exchange?.timing.providerLatencyMs ??
                  "Not returned by the gateway",
              }}
            />
          </TabsContent>
          <TabsContent value="routing" className="min-h-0 overflow-y-auto">
            <JsonBlock value={exchange?.routing} />
          </TabsContent>
        </Tabs>
      </SheetContent>
    </Sheet>
  )
}

function CodeSheet({
  open,
  onOpenChange,
  examples,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  examples: Record<string, string>
}) {
  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent className="w-[min(520px,94vw)] sm:max-w-[520px]">
        <SheetHeader className="border-b p-4">
          <SheetTitle>View code</SheetTitle>
          <SheetDescription>
            Uses the Tuenel endpoint, current alias, and an environment variable
            for credentials.
          </SheetDescription>
        </SheetHeader>
        <Tabs defaultValue="curl" className="min-h-0 flex-1 p-4">
          <TabsList>
            <TabsTrigger value="curl">cURL</TabsTrigger>
            <TabsTrigger value="python">Python</TabsTrigger>
            <TabsTrigger value="javascript">JavaScript</TabsTrigger>
            <TabsTrigger value="http">HTTP</TabsTrigger>
          </TabsList>
          {Object.entries(examples).map(([name, code]) => (
            <TabsContent key={name} value={name}>
              <div className="mb-2 flex justify-end">
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() =>
                    navigator.clipboard
                      .writeText(code)
                      .then(() => toast.success("Code copied"))
                  }
                >
                  <CopyIcon data-icon="inline-start" />
                  Copy
                </Button>
              </div>
              <pre className="max-h-[calc(100dvh-12rem)] overflow-auto rounded-md border bg-muted/30 p-3 text-[11px] whitespace-pre-wrap">
                {code}
              </pre>
            </TabsContent>
          ))}
        </Tabs>
      </SheetContent>
    </Sheet>
  )
}
