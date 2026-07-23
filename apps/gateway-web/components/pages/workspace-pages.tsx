"use client"

import * as React from "react"
import { CopyIcon, PlayIcon, StopIcon, TrashIcon } from "@phosphor-icons/react"
import { toast } from "sonner"

import { useGateway } from "@/components/gateway-provider"
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
import { Spinner } from "@/components/ui/spinner"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { Textarea } from "@/components/ui/textarea"
import {
  type Page,
  gatewayFetch,
  gatewayResponse,
  readSse,
} from "@/lib/gateway-api"

type JsonRecord = Record<string, unknown>

export function OverviewPage({ operator = false }: { operator?: boolean }) {
  const path = operator ? "/admin/summary" : "/admin/usage/summary"
  const state = useGatewayData<JsonRecord>(path)
  const usage = (state.data?.usage ?? state.data ?? {}) as JsonRecord
  return (
    <>
      <PageHeader
        title={operator ? "Gateway overview" : "Workspace overview"}
        description="A current snapshot from the gateway control plane."
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
      <PageHeader
        title="Models"
        description="Public model aliases currently exposed by the live gateway."
      />
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
  const { tenantId } = useGateway()
  const [operation, setOperation] = React.useState("chat")
  const [model, setModel] = React.useState("")
  const [input, setInput] = React.useState("")
  const [output, setOutput] = React.useState("")
  const [running, setRunning] = React.useState(false)
  const controller = React.useRef<AbortController>(null)
  const models = useGatewayData<{ data: { id: string }[] }>("/v1/models")

  const selectedModel = model || models.data?.data[0]?.id || ""

  async function run() {
    controller.current?.abort()
    const abort = new AbortController()
    controller.current = abort
    setRunning(true)
    setOutput("")
    try {
      if (operation === "embeddings") {
        const value = await gatewayFetch<JsonRecord>(
          "/v1/embeddings",
          tenantId,
          {
            method: "POST",
            headers: { "content-type": "application/json" },
            body: JSON.stringify({ model: selectedModel, input }),
            signal: abort.signal,
          }
        )
        setOutput(JSON.stringify(value, null, 2))
        return
      }
      const responses = operation === "responses"
      const response = await gatewayResponse(
        responses ? "/v1/responses" : "/v1/chat/completions",
        tenantId,
        {
          method: "POST",
          headers: {
            "content-type": "application/json",
            accept: "text/event-stream",
          },
          body: JSON.stringify(
            responses
              ? { model: selectedModel, input, stream: true }
              : {
                  model: selectedModel,
                  messages: [{ role: "user", content: input }],
                  stream: true,
                }
          ),
          signal: abort.signal,
        }
      )
      await readSse(
        response,
        (event) => {
          const value = event as JsonRecord
          const choices = value.choices as
            { delta?: { content?: string } }[] | undefined
          const delta =
            choices?.[0]?.delta?.content ??
            (value.type === "response.output_text.delta" ? value.delta : "")
          if (typeof delta === "string") setOutput((current) => current + delta)
        },
        abort.signal
      )
    } catch (error) {
      if (!abort.signal.aborted)
        toast.error(error instanceof Error ? error.message : "Request failed")
    } finally {
      setRunning(false)
      controller.current = null
    }
  }

  return (
    <>
      <PageHeader
        title="Playground"
        description="Send abortable streaming inference and embedding requests."
      />
      <div className="grid gap-4 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>Request</CardTitle>
            <CardDescription>
              Plaintext input remains in this tab only.
            </CardDescription>
          </CardHeader>
          <CardContent>
            <FieldGroup>
              <Field>
                <FieldLabel>Operation</FieldLabel>
                <Tabs value={operation} onValueChange={setOperation}>
                  <TabsList>
                    <TabsTrigger value="chat">Chat</TabsTrigger>
                    <TabsTrigger value="responses">Responses</TabsTrigger>
                    <TabsTrigger value="embeddings">Embeddings</TabsTrigger>
                  </TabsList>
                </Tabs>
              </Field>
              <Field>
                <FieldLabel htmlFor="playground-model">Model</FieldLabel>
                <Input
                  id="playground-model"
                  value={selectedModel}
                  onChange={(event) => setModel(event.target.value)}
                  required
                />
              </Field>
              <Field>
                <FieldLabel htmlFor="playground-input">Input</FieldLabel>
                <Textarea
                  id="playground-input"
                  value={input}
                  onChange={(event) => setInput(event.target.value)}
                  rows={10}
                />
              </Field>
              <div className="flex gap-2">
                <Button
                  disabled={running || !selectedModel || !input}
                  onClick={run}
                >
                  {running ? (
                    <Spinner data-icon="inline-start" />
                  ) : (
                    <PlayIcon data-icon="inline-start" />
                  )}
                  Run
                </Button>
                <Button
                  variant="outline"
                  disabled={!running}
                  onClick={() => controller.current?.abort()}
                >
                  <StopIcon data-icon="inline-start" />
                  Stop
                </Button>
              </div>
            </FieldGroup>
          </CardContent>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle>Response</CardTitle>
            <CardDescription>
              Direct gateway output without fabricated usage.
            </CardDescription>
          </CardHeader>
          <CardContent>
            <pre className="min-h-64 overflow-auto rounded-md bg-muted p-4 text-sm whitespace-pre-wrap">
              {output || "Run a request to see the response."}
            </pre>
          </CardContent>
        </Card>
      </div>
    </>
  )
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
  const { tenantId } = useGateway()
  const state = useGatewayData<Page<VirtualKey>>("/admin/virtual-keys")
  const [issued, setIssued] = React.useState<string>()
  const [name, setName] = React.useState("")

  return (
    <>
      <PageHeader
        title="Virtual keys"
        description="Issue and revoke durable tenant credentials. Plaintext is shown once."
      />
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
                            `/admin/virtual-keys/${key.id}`,
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
  const summary = useGatewayData<JsonRecord>("/admin/usage/summary")
  const events = useGatewayData<Page<JsonRecord>>("/admin/usage/events")
  return (
    <>
      <PageHeader
        title="Usage and cost"
        description="PostgreSQL-backed usage totals and immutable events."
      />
      <DataState
        loading={summary.loading}
        error={summary.error}
        onRetry={summary.reload}
      >
        <div className="mb-4 grid gap-4 md:grid-cols-3">
          <Metric
            label="Requests"
            value={String(summary.data?.requests ?? 0)}
            detail="Completed gateway events"
          />
          <Metric
            label="Tokens"
            value={String(summary.data?.total_tokens ?? 0)}
            detail="Input plus output"
          />
          <Metric
            label="Estimated cost"
            value={`$${summary.data?.estimated_cost ?? 0}`}
            detail="Active pricing catalog"
          />
        </div>
      </DataState>
      <RecordTable state={events} />
    </>
  )
}

export function DocsPage() {
  const state = useGatewayData<JsonRecord>("/openapi.json")
  return (
    <>
      <PageHeader
        title="API documentation"
        description="The live OpenAPI document generated by the running gateway."
      />
      <DataState
        loading={state.loading}
        error={state.error}
        onRetry={state.reload}
      >
        <Card>
          <CardContent>
            <pre className="max-h-[70vh] overflow-auto text-xs whitespace-pre-wrap">
              {JSON.stringify(state.data, null, 2)}
            </pre>
          </CardContent>
        </Card>
      </DataState>
    </>
  )
}

function RecordTable({
  state,
}: {
  state: ReturnType<typeof useGatewayData<Page<JsonRecord>>>
}) {
  const records = state.data?.data ?? []
  return (
    <DataState
      loading={state.loading}
      error={state.error}
      empty={records.length === 0}
      onRetry={state.reload}
    >
      <Card>
        <CardContent>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Record</TableHead>
                <TableHead>Status</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {records.map((record, index) => (
                <TableRow key={String(record.id ?? record.event_id ?? index)}>
                  <TableCell>
                    <pre className="max-w-4xl overflow-auto text-xs whitespace-pre-wrap">
                      {JSON.stringify(record, null, 2)}
                    </pre>
                  </TableCell>
                  <TableCell>
                    {typeof record.status === "string" ? (
                      <StatusBadge status={record.status} />
                    ) : (
                      <Badge variant="outline">Recorded</Badge>
                    )}
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </CardContent>
      </Card>
    </DataState>
  )
}
