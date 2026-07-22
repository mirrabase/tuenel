"use client"

import * as React from "react"
import { toast } from "sonner"
import {
  ClipboardIcon,
  KeyIcon,
  LockKeyIcon,
  PaperPlaneTiltIcon,
  TrashIcon,
  WarningCircleIcon,
} from "@phosphor-icons/react"

import { useMockGateway } from "@/components/mock-provider"
import {
  BackendNotice,
  Metric,
  PageHeader,
  StateVariants,
  StatusBadge,
} from "@/components/pages/shared"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from "@/components/ui/alert-dialog"
import { Badge } from "@/components/ui/badge"
import { Bubble, BubbleContent } from "@/components/ui/bubble"
import { Button } from "@/components/ui/button"
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog"
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import {
  Message,
  MessageContent,
  MessageFooter,
  MessageHeader,
} from "@/components/ui/message"
import {
  MessageScroller,
  MessageScrollerButton,
  MessageScrollerContent,
  MessageScrollerItem,
  MessageScrollerProvider,
  MessageScrollerViewport,
} from "@/components/ui/message-scroller"
import {
  Select,
  SelectContent,
  SelectGroup,
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
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { Textarea } from "@/components/ui/textarea"
import { gatewayFetch } from "@/lib/gateway-api"
import { usageEvents } from "@/lib/mock-data"

export function OverviewPage({ operator = false }: { operator?: boolean }) {
  return (
    <>
      <PageHeader
        title={operator ? "Fleet overview" : "Workspace overview"}
        description="Deterministic Gateway v0.3 operational snapshot; every value on this page is mock data."
      />
      <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
        <Metric
          label="Requests · 24h"
          value={operator ? "50.8k" : "24.8k"}
          detail="Simulated traffic"
        />
        <Metric
          label="Success rate"
          value="99.72%"
          detail="Simulated provider outcomes"
        />
        <Metric
          label="Token quota"
          value="68%"
          detail="Mock reservation counters"
        />
        <Metric
          label="Estimated cost"
          value="$318.20"
          detail="Mock pricing snapshot"
        />
      </div>
      <Card className="mt-4">
        <CardHeader>
          <CardTitle>Recent requests</CardTitle>
          <CardDescription>Sanitized mock usage events.</CardDescription>
        </CardHeader>
        <CardContent>
          <UsageTable />
        </CardContent>
      </Card>
    </>
  )
}

function UsageTable() {
  return (
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead>Request</TableHead>
          <TableHead>Model</TableHead>
          <TableHead>Status</TableHead>
          <TableHead>Tokens</TableHead>
          <TableHead>Cost</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {usageEvents.map((row) => (
          <TableRow key={row.id}>
            <TableCell className="font-mono">{row.id}</TableCell>
            <TableCell>{row.model}</TableCell>
            <TableCell>
              <StatusBadge status={row.status} />
            </TableCell>
            <TableCell>{row.tokens.toLocaleString()}</TableCell>
            <TableCell>{row.cost}</TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
  )
}

const scenarioItems = ["safe", "warn", "redact", "block"] as const
export function PlaygroundPage() {
  const { state } = useMockGateway()
  const [surface, setSurface] = React.useState("chat")
  const [scenario, setScenario] =
    React.useState<(typeof scenarioItems)[number]>("safe")
  const [prompt, setPrompt] = React.useState(
    "Explain why an AI gateway is useful."
  )
  const [answer, setAnswer] = React.useState("")
  const [running, setRunning] = React.useState(false)
  const run = async () => {
    if (!prompt.trim()) return
    setRunning(true)
    setAnswer("")
    try {
      const tenant = state.principal!.tenantId
      if (surface === "embeddings") {
        const result = await gatewayFetch<{
          data: Array<{ embedding: number[] }>
        }>("/v1/embeddings", tenant, {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({ model: "gateway-default", input: prompt }),
        })
        setAnswer(JSON.stringify(result.data[0]?.embedding ?? []))
      } else if (surface === "responses") {
        const result = await gatewayFetch<{
          output: Array<{ content: Array<{ text: string }> }>
        }>("/v1/responses", tenant, {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({
            model: "gateway-default",
            input: prompt,
            stream: false,
          }),
        })
        setAnswer(result.output?.[0]?.content?.[0]?.text ?? "")
      } else {
        const result = await gatewayFetch<{
          choices: Array<{ message: { content: string } }>
        }>("/v1/chat/completions", tenant, {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({
            model: "gateway-default",
            messages: [{ role: "user", content: prompt }],
            stream: false,
          }),
        })
        setAnswer(result.choices?.[0]?.message?.content ?? "")
      }
    } catch (error) {
      setAnswer(
        error instanceof Error ? error.message : "Gateway request failed"
      )
    } finally {
      setRunning(false)
    }
  }
  const payload =
    surface === "responses"
      ? { model: "gateway-default", input: prompt, stream: true }
      : surface === "embeddings"
        ? { model: "text-embedding-demo", input: prompt }
        : {
            model: "gateway-default",
            messages: [{ role: "user", content: prompt }],
            stream: true,
          }
  return (
    <>
      <PageHeader
        title="Playground"
        description="Run Chat Completions, Responses, and Embeddings through the authenticated gateway."
      />
      <BackendNotice />
      <Tabs value={surface} onValueChange={setSurface} className="mt-4">
        <TabsList>
          <TabsTrigger value="chat">Chat Completions</TabsTrigger>
          <TabsTrigger value="responses">Responses</TabsTrigger>
          <TabsTrigger value="embeddings">Embeddings</TabsTrigger>
        </TabsList>
        <TabsContent value={surface} className="pt-3">
          <div className="grid gap-4 xl:grid-cols-[minmax(0,1fr)_360px]">
            <Card>
              <CardHeader>
                <CardTitle>Messages</CardTitle>
                <CardDescription>
                  Streaming and inspection are simulated locally.
                </CardDescription>
              </CardHeader>
              <CardContent className="flex h-[480px] flex-col gap-3">
                <MessageScrollerProvider autoScroll>
                  <MessageScroller>
                    <MessageScrollerViewport>
                      <MessageScrollerContent>
                        <MessageScrollerItem messageId="user" scrollAnchor>
                          <Message align="end">
                            <MessageContent>
                              <MessageHeader>You</MessageHeader>
                              <Bubble align="end">
                                <BubbleContent>{prompt}</BubbleContent>
                              </Bubble>
                            </MessageContent>
                          </Message>
                        </MessageScrollerItem>
                        {answer && (
                          <MessageScrollerItem messageId="assistant">
                            <Message>
                              <MessageContent>
                                <MessageHeader>Gateway mock</MessageHeader>
                                <Bubble
                                  variant={
                                    scenario === "block"
                                      ? "destructive"
                                      : "muted"
                                  }
                                >
                                  <BubbleContent>
                                    <pre className="font-sans whitespace-pre-wrap">
                                      {answer}
                                    </pre>
                                  </BubbleContent>
                                </Bubble>
                                <MessageFooter>
                                  82 tokens · 842ms · {scenario}
                                </MessageFooter>
                              </MessageContent>
                            </Message>
                          </MessageScrollerItem>
                        )}
                      </MessageScrollerContent>
                    </MessageScrollerViewport>
                    <MessageScrollerButton />
                  </MessageScroller>
                </MessageScrollerProvider>
                <Textarea
                  value={prompt}
                  onChange={(event) => setPrompt(event.target.value)}
                  aria-label="Prompt"
                />
                <Button onClick={run} disabled={running || !prompt.trim()}>
                  <PaperPlaneTiltIcon data-icon="inline-start" />
                  {running ? "Running…" : "Run request"}
                </Button>
              </CardContent>
            </Card>
            <div className="flex flex-col gap-4">
              <Card>
                <CardHeader>
                  <CardTitle>Request</CardTitle>
                  <CardDescription>
                    OpenAI-compatible mock payload
                  </CardDescription>
                </CardHeader>
                <CardContent>
                  <pre className="overflow-x-auto rounded-md bg-muted p-3 text-xs">
                    {JSON.stringify(payload, null, 2)}
                  </pre>
                </CardContent>
              </Card>
              <Card>
                <CardHeader>
                  <CardTitle>Inspection scenario</CardTitle>
                </CardHeader>
                <CardContent>
                  <Select
                    items={scenarioItems.map((value) => ({
                      label: value,
                      value,
                    }))}
                    value={scenario}
                    onValueChange={(value) =>
                      value && setScenario(value as typeof scenario)
                    }
                  >
                    <SelectTrigger className="w-full">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectGroup>
                        {scenarioItems.map((value) => (
                          <SelectItem key={value} value={value}>
                            {value}
                          </SelectItem>
                        ))}
                      </SelectGroup>
                    </SelectContent>
                  </Select>
                  <div className="mt-3 flex flex-wrap gap-1">
                    <Badge variant="outline">request inspection</Badge>
                    <Badge variant="outline">response inspection</Badge>
                    <Badge variant="outline">usage envelope</Badge>
                  </div>
                </CardContent>
              </Card>
            </div>
          </div>
        </TabsContent>
      </Tabs>
    </>
  )
}

export function ModelsPage() {
  const { state } = useMockGateway()
  const [items, setItems] = React.useState<
    Array<{ id: string; owned_by: string }>
  >([])
  const [error, setError] = React.useState<string>()
  React.useEffect(() => {
    gatewayFetch<{ data: Array<{ id: string; owned_by: string }> }>(
      "/v1/models",
      state.principal!.tenantId
    )
      .then((result) => setItems(result.data))
      .catch((value) =>
        setError(
          value instanceof Error ? value.message : "Gateway request failed"
        )
      )
  }, [state.principal])
  return (
    <>
      <PageHeader
        title="Models"
        description="Stable model aliases available to the current tenant."
      />
      {error ? (
        <Alert variant="destructive">
          <WarningCircleIcon />
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      ) : (
        <div className="grid gap-4 lg:grid-cols-3">
          {items.map((model) => (
            <Card key={model.id}>
              <CardHeader>
                <CardTitle>{model.id}</CardTitle>
                <CardDescription>{model.owned_by}</CardDescription>
                <CardAction>
                  <StatusBadge status="available" />
                </CardAction>
              </CardHeader>
              <CardContent className="flex flex-col gap-2">
                <span className="text-muted-foreground">
                  Provider-neutral gateway alias
                </span>
              </CardContent>
            </Card>
          ))}
        </div>
      )}
    </>
  )
}

export function KeysPage() {
  const { state } = useMockGateway()
  const [open, setOpen] = React.useState(false)
  const [revealedSecret, setRevealedSecret] = React.useState<string | null>(
    null
  )
  const [keys, setKeys] = React.useState<
    Array<{
      id: string
      name: string
      prefix: string
      quota: string
      status: "Active" | "Revoked"
    }>
  >([])
  return (
    <>
      <PageHeader
        title="Virtual Keys"
        description="Tenant-scoped Virtual Keys. Plaintext is returned once and retained only in memory."
        action={
          <Dialog
            open={open}
            onOpenChange={(value) => {
              setOpen(value)
              if (!value) setRevealedSecret(null)
            }}
          >
            <DialogTrigger render={<Button />}>
              <KeyIcon data-icon="inline-start" />
              Issue key
            </DialogTrigger>
            <DialogContent>
              <DialogHeader>
                <DialogTitle>
                  {revealedSecret ? "Copy key" : "Issue Virtual Key"}
                </DialogTitle>
                <DialogDescription>
                  Store it now. The plaintext will not be shown again.
                </DialogDescription>
              </DialogHeader>
              {revealedSecret ? (
                <>
                  <Alert>
                    <LockKeyIcon />
                    <AlertTitle>Shown once</AlertTitle>
                    <AlertDescription>{revealedSecret}</AlertDescription>
                  </Alert>
                  <Button
                    variant="outline"
                    onClick={() =>
                      navigator.clipboard
                        .writeText(revealedSecret)
                        .then(() => toast.success("Key copied"))
                    }
                  >
                    <ClipboardIcon data-icon="inline-start" />
                    Copy demo key
                  </Button>
                </>
              ) : (
                <form
                  onSubmit={async (event) => {
                    event.preventDefault()
                    const form = new FormData(event.currentTarget)
                    try {
                      const expiry = String(form.get("expiry") ?? "")
                      const result = await gatewayFetch<{
                        id: string
                        key: string
                        key_prefix: string
                        daily_token_limit: number
                      }>("/admin/virtual-keys", state.principal!.tenantId, {
                        method: "POST",
                        headers: { "content-type": "application/json" },
                        body: JSON.stringify({
                          scopes: ["chat", "responses", "embeddings"],
                          expires_at: expiry
                            ? new Date(expiry).toISOString()
                            : null,
                        }),
                      })
                      setRevealedSecret(result.key)
                      setKeys((items) => [
                        ...items,
                        {
                          id: result.id,
                          name: String(form.get("name")),
                          prefix: result.key_prefix,
                          quota: String(result.daily_token_limit),
                          status: "Active",
                        },
                      ])
                    } catch (error) {
                      toast.error(
                        error instanceof Error
                          ? error.message
                          : "Key issuance failed"
                      )
                    }
                  }}
                >
                  <FieldGroup>
                    <Field>
                      <FieldLabel htmlFor="key-name">Name</FieldLabel>
                      <Input
                        id="key-name"
                        name="name"
                        required
                        defaultValue="New integration"
                      />
                    </Field>
                    <Field>
                      <FieldLabel htmlFor="key-expiry">Expiry</FieldLabel>
                      <Input
                        id="key-expiry"
                        name="expiry"
                        type="datetime-local"
                      />
                    </Field>
                  </FieldGroup>
                  <DialogFooter className="mt-4">
                    <DialogClose render={<Button variant="outline" />}>
                      Cancel
                    </DialogClose>
                    <Button type="submit">Issue key</Button>
                  </DialogFooter>
                </form>
              )}
            </DialogContent>
          </Dialog>
        }
      />
      <Card>
        <CardHeader>
          <CardTitle>Issued keys</CardTitle>
          <CardDescription>
            This session shows only keys issued since the page was opened
            because listing is not available yet.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Name</TableHead>
                <TableHead>Prefix</TableHead>
                <TableHead>Quota</TableHead>
                <TableHead>Status</TableHead>
                <TableHead>Action</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {keys.map((key) => (
                <TableRow key={key.id}>
                  <TableCell>{key.name}</TableCell>
                  <TableCell className="font-mono">{key.prefix}</TableCell>
                  <TableCell>{key.quota}</TableCell>
                  <TableCell>
                    <StatusBadge status={key.status} />
                  </TableCell>
                  <TableCell>
                    {key.status === "Active" && (
                      <AlertDialog>
                        <AlertDialogTrigger
                          render={
                            <Button
                              variant="ghost"
                              size="icon-sm"
                              aria-label={`Revoke ${key.name}`}
                            />
                          }
                        >
                          <TrashIcon />
                        </AlertDialogTrigger>
                        <AlertDialogContent>
                          <AlertDialogHeader>
                            <AlertDialogTitle>
                              Revoke {key.name}?
                            </AlertDialogTitle>
                            <AlertDialogDescription>
                              The key will stop authenticating immediately.
                            </AlertDialogDescription>
                          </AlertDialogHeader>
                          <AlertDialogFooter>
                            <AlertDialogCancel>Cancel</AlertDialogCancel>
                            <AlertDialogAction
                              variant="destructive"
                              onClick={async () => {
                                try {
                                  await gatewayFetch(
                                    `/admin/virtual-keys/${key.id}`,
                                    state.principal!.tenantId,
                                    { method: "DELETE" }
                                  )
                                  setKeys((items) =>
                                    items.map((item) =>
                                      item.id === key.id
                                        ? { ...item, status: "Revoked" }
                                        : item
                                    )
                                  )
                                } catch (error) {
                                  toast.error(
                                    error instanceof Error
                                      ? error.message
                                      : "Revocation failed"
                                  )
                                }
                              }}
                            >
                              Revoke key
                            </AlertDialogAction>
                          </AlertDialogFooter>
                        </AlertDialogContent>
                      </AlertDialog>
                    )}
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </CardContent>
      </Card>
    </>
  )
}

export function UsagePage() {
  return (
    <>
      <PageHeader
        title="Usage & cost"
        description="Mock request usage, quota, cost, and reservation query surfaces."
      />
      <div className="grid gap-3 sm:grid-cols-3">
        <Metric
          label="Tokens · 7d"
          value="763.5k"
          detail="534.1k input · 229.4k output"
        />
        <Metric
          label="Estimated cost"
          value="$82.41"
          detail="Mock normalized pricing"
        />
        <Metric
          label="Quota remaining"
          value="32%"
          detail="Mock Redis-style counter"
        />
      </div>
      <Card className="mt-4">
        <CardHeader>
          <CardTitle>Request ledger</CardTitle>
          <CardDescription>
            Backend query API is not yet available.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <StateVariants>
            <UsageTable />
          </StateVariants>
        </CardContent>
      </Card>
    </>
  )
}

const endpoints = [
  ["GET", "/health", "Machine health"],
  ["GET", "/ready", "Machine readiness"],
  ["GET", "/metrics", "Machine metrics"],
  ["GET", "/openapi.json", "Schema"],
  ["GET", "/v1/models", "Models"],
  ["POST", "/v1/chat/completions", "Chat"],
  ["POST", "/v1/responses", "Responses"],
  ["POST", "/v1/embeddings", "Embeddings"],
  ["GET", "/v1/mcp/servers", "Permitted MCP servers"],
  ["GET", "/v1/mcp/tools", "Permitted tools"],
  ["POST", "/v1/mcp/tools/call", "Tool invocation"],
  ["POST", "/mcp", "Native machine-to-machine MCP"],
]

export function DocsPage() {
  return (
    <>
      <PageHeader
        title="API docs"
        description="Gateway v0.3 endpoint catalog and live OpenAPI surface."
      />
      <Alert>
        <WarningCircleIcon />
        <AlertTitle>Authenticated BFF enabled</AlertTitle>
        <AlertDescription>
          Browser credentials remain in an encrypted HttpOnly cookie. Native
          /mcp remains machine-to-machine.
        </AlertDescription>
      </Alert>
      <Card className="mt-4">
        <CardHeader>
          <CardTitle>Endpoints</CardTitle>
          <CardDescription>
            Available routes are forwarded through the same-origin web boundary.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Method</TableHead>
                <TableHead>Path</TableHead>
                <TableHead>Surface</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {endpoints.map(([method, path, surface]) => (
                <TableRow key={`${method}${path}`}>
                  <TableCell>
                    <Badge variant="outline">{method}</Badge>
                  </TableCell>
                  <TableCell className="font-mono">{path}</TableCell>
                  <TableCell>{surface}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </CardContent>
      </Card>
    </>
  )
}
