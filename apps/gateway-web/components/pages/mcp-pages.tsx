"use client"

import * as React from "react"
import { toast } from "sonner"
import {
  ArrowsClockwiseIcon,
  CheckCircleIcon,
  EyeIcon,
  PlusIcon,
  PulseIcon,
  TrashIcon,
  WarningCircleIcon,
} from "@phosphor-icons/react"

import { useMockGateway } from "@/components/mock-provider"
import {
  BackendNotice,
  PageHeader,
  StatusBadge,
} from "@/components/pages/shared"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Badge } from "@/components/ui/badge"
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
  SelectGroup,
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
  SheetTrigger,
} from "@/components/ui/sheet"
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
import {
  visibleApprovals,
  type Invocation,
  type McpPolicy,
} from "@/lib/mock-store"

function ServerForm() {
  const { state, dispatch } = useMockGateway()
  const [transport, setTransport] = React.useState<"http" | "stdio">("http")
  return (
    <form
      onSubmit={(event) => {
        event.preventDefault()
        const form = new FormData(event.currentTarget)
        dispatch({
          type: "server.create",
          server: {
            tenantId: state.principal!.tenantId,
            name: String(form.get("name")),
            transport,
            endpoint:
              transport === "http" ? String(form.get("endpoint")) : undefined,
            command:
              transport === "stdio" ? String(form.get("command")) : undefined,
            enabled: true,
          },
        })
        toast.success("Mock MCP server created")
      }}
    >
      <FieldGroup>
        <Field>
          <FieldLabel htmlFor="server-name">Name</FieldLabel>
          <Input id="server-name" name="name" required />
        </Field>
        <Field>
          <FieldLabel>Transport</FieldLabel>
          <Select
            items={[
              { label: "HTTP", value: "http" },
              { label: "stdio", value: "stdio" },
            ]}
            value={transport}
            onValueChange={(value) =>
              value && setTransport(value as "http" | "stdio")
            }
          >
            <SelectTrigger className="w-full">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectGroup>
                <SelectItem value="http">HTTP</SelectItem>
                <SelectItem value="stdio">stdio</SelectItem>
              </SelectGroup>
            </SelectContent>
          </Select>
        </Field>
        {transport === "http" ? (
          <Field>
            <FieldLabel htmlFor="server-endpoint">HTTPS endpoint</FieldLabel>
            <Input
              id="server-endpoint"
              name="endpoint"
              type="url"
              required
              defaultValue="https://demo.invalid/mcp"
            />
          </Field>
        ) : (
          <>
            <Field>
              <FieldLabel htmlFor="server-command">Command</FieldLabel>
              <Input
                id="server-command"
                name="command"
                required
                defaultValue="demo-mcp-server"
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="server-args">Arguments</FieldLabel>
              <Input id="server-args" placeholder="--demo" />
            </Field>
          </>
        )}
        <Field>
          <FieldLabel htmlFor="server-credential">
            Credential / environment (write-only)
          </FieldLabel>
          <Input
            id="server-credential"
            type="password"
            autoComplete="off"
            placeholder="demo value, never stored"
          />
          <FieldDescription>
            This field is discarded after simulated submit.
          </FieldDescription>
        </Field>
      </FieldGroup>
      <DialogFooter className="mt-4">
        <DialogClose render={<Button variant="outline" />}>Cancel</DialogClose>
        <DialogClose render={<Button type="submit" />}>
          Create mock server
        </DialogClose>
      </DialogFooter>
    </form>
  )
}

function ServerEditForm({ id }: { id: string }) {
  const { state, dispatch } = useMockGateway()
  const server = state.servers[id]
  return (
    <form
      onSubmit={(event) => {
        event.preventDefault()
        const form = new FormData(event.currentTarget)
        dispatch({
          type: "server.update",
          id,
          patch: {
            name: String(form.get("name")),
            endpoint:
              server.transport === "http"
                ? String(form.get("target"))
                : undefined,
            command:
              server.transport === "stdio"
                ? String(form.get("target"))
                : undefined,
          },
        })
      }}
    >
      <FieldGroup>
        <Field>
          <FieldLabel htmlFor={`edit-name-${id}`}>Name</FieldLabel>
          <Input
            id={`edit-name-${id}`}
            name="name"
            defaultValue={server.name}
            required
          />
        </Field>
        <Field>
          <FieldLabel htmlFor={`edit-target-${id}`}>
            {server.transport === "http" ? "HTTPS endpoint" : "Command"}
          </FieldLabel>
          <Input
            id={`edit-target-${id}`}
            name="target"
            defaultValue={server.endpoint ?? server.command}
            required
          />
        </Field>
        <Field>
          <FieldLabel htmlFor={`edit-secret-${id}`}>
            Replacement credential / environment (write-only)
          </FieldLabel>
          <Input
            id={`edit-secret-${id}`}
            type="password"
            autoComplete="off"
            placeholder="Optional demo value, discarded"
          />
        </Field>
      </FieldGroup>
      <DialogFooter className="mt-4">
        <DialogClose render={<Button variant="outline" />}>Cancel</DialogClose>
        <DialogClose render={<Button type="submit" />}>
          Save mock server
        </DialogClose>
      </DialogFooter>
    </form>
  )
}

export function McpRegistryPage() {
  const { state, dispatch } = useMockGateway()
  const servers = Object.values(state.servers).filter(
    (server) => server.tenantId === state.principal?.tenantId
  )
  return (
    <>
      <PageHeader
        title="MCP registry"
        description="Simulated server lifecycle, health, discovery, inventory, and write-only credentials."
        action={
          <Dialog>
            <DialogTrigger render={<Button />}>
              <PlusIcon data-icon="inline-start" />
              Add mock server
            </DialogTrigger>
            <DialogContent>
              <DialogHeader>
                <DialogTitle>Create MCP server</DialogTitle>
                <DialogDescription>
                  HTTP and stdio configuration are held in memory only.
                </DialogDescription>
              </DialogHeader>
              <ServerForm />
            </DialogContent>
          </Dialog>
        }
      />
      <BackendNotice />
      <div className="mt-4 grid gap-4 lg:grid-cols-2">
        {servers.map((server) => (
          <Card key={server.id}>
            <CardHeader>
              <CardTitle>{server.name}</CardTitle>
              <CardDescription>
                {server.transport === "http" ? server.endpoint : server.command}
              </CardDescription>
              <CardAction>
                <StatusBadge status={server.health} />
              </CardAction>
            </CardHeader>
            <CardContent className="flex flex-col gap-3">
              <div className="flex flex-wrap gap-2">
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() =>
                    dispatch({ type: "server.health", id: server.id })
                  }
                >
                  <PulseIcon data-icon="inline-start" />
                  Simulate health
                </Button>
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() =>
                    dispatch({ type: "server.refresh", id: server.id })
                  }
                >
                  <ArrowsClockwiseIcon data-icon="inline-start" />
                  Refresh mock discovery
                </Button>
                <Dialog>
                  <DialogTrigger
                    render={<Button size="sm" variant="outline" />}
                  >
                    Edit mock server
                  </DialogTrigger>
                  <DialogContent>
                    <DialogHeader>
                      <DialogTitle>Edit {server.name}</DialogTitle>
                      <DialogDescription>
                        Changes and replacement credentials remain in browser
                        memory.
                      </DialogDescription>
                    </DialogHeader>
                    <ServerEditForm id={server.id} />
                  </DialogContent>
                </Dialog>
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() =>
                    dispatch({
                      type: "server.update",
                      id: server.id,
                      patch: {
                        enabled: !server.enabled,
                        health: server.enabled ? "disabled" : "degraded",
                      },
                    })
                  }
                >
                  {server.enabled
                    ? "Disable mock server"
                    : "Enable mock server"}
                </Button>
                <Button
                  size="sm"
                  variant="destructive"
                  onClick={() =>
                    dispatch({ type: "server.delete", id: server.id })
                  }
                >
                  <TrashIcon data-icon="inline-start" />
                  Delete mock server
                </Button>
              </div>
              <p className="text-muted-foreground">
                {server.toolIds.length} tools · refreshed {server.refreshedAt}
              </p>
              <div className="flex flex-col gap-2">
                {server.toolIds.map((toolId) => {
                  const tool = state.tools[toolId]
                  return tool ? (
                    <Sheet key={tool.id}>
                      <SheetTrigger
                        render={
                          <Button variant="ghost" className="justify-between" />
                        }
                      >
                        <span>{tool.name}</span>
                        <EyeIcon data-icon="inline-end" />
                      </SheetTrigger>
                      <SheetContent>
                        <SheetHeader>
                          <SheetTitle>{tool.name}</SheetTitle>
                          <SheetDescription>
                            {tool.description}
                          </SheetDescription>
                        </SheetHeader>
                        <div className="flex flex-col gap-4 p-4">
                          <Badge variant="outline">{tool.risk} risk</Badge>
                          <pre className="overflow-x-auto rounded-md bg-muted p-3 text-xs">
                            {JSON.stringify(tool.schema, null, 2)}
                          </pre>
                          <Alert>
                            <WarningCircleIcon />
                            <AlertTitle>
                              Risk annotation is mock metadata
                            </AlertTitle>
                            <AlertDescription>
                              Direct annotation CRUD is not available in the
                              backend.
                            </AlertDescription>
                          </Alert>
                        </div>
                      </SheetContent>
                    </Sheet>
                  ) : null
                })}
              </div>
            </CardContent>
          </Card>
        ))}
      </div>
    </>
  )
}

const defaultPolicy = (tenantId: string): McpPolicy => ({
  id: `pol-demo-${tenantId}`,
  name: "New MCP policy",
  tenantId,
  projectId: undefined,
  serverRule: "allow",
  toolRule: "allow",
  action: "approval",
  riskOverride: "inherit",
  argumentRule: "JSON arguments must match schema",
  rpm: 30,
  daily: 500,
  concurrency: 3,
  maxBytes: 65536,
  timeoutMs: 15000,
})

function PolicyForm({ value }: { value: McpPolicy }) {
  const { dispatch } = useMockGateway()
  const [policy, setPolicy] = React.useState(value)
  const select = <K extends keyof McpPolicy>(key: K, next: McpPolicy[K]) =>
    setPolicy((current) => ({ ...current, [key]: next }))
  return (
    <form
      onSubmit={(event) => {
        event.preventDefault()
        dispatch({ type: "policy.save", policy })
        toast.success("Mock MCP policy saved")
      }}
    >
      <FieldGroup>
        <Field>
          <FieldLabel htmlFor={`policy-name-${policy.id}`}>Name</FieldLabel>
          <Input
            id={`policy-name-${policy.id}`}
            value={policy.name}
            onChange={(event) => select("name", event.target.value)}
          />
        </Field>
        <Field>
          <FieldLabel htmlFor={`policy-project-${policy.id}`}>
            Scope hierarchy
          </FieldLabel>
          <Input
            id={`policy-project-${policy.id}`}
            value={policy.projectId ?? "tenant-wide"}
            onChange={(event) =>
              select(
                "projectId",
                event.target.value === "tenant-wide"
                  ? undefined
                  : event.target.value
              )
            }
          />
          <FieldDescription>
            Tenant → project → key; most specific mock rule wins.
          </FieldDescription>
        </Field>
        <div className="grid gap-3 sm:grid-cols-2">
          <Field>
            <FieldLabel>Server allow/deny</FieldLabel>
            <Select
              items={[
                { label: "allow", value: "allow" },
                { label: "deny", value: "deny" },
              ]}
              value={policy.serverRule}
              onValueChange={(value) =>
                value && select("serverRule", value as McpPolicy["serverRule"])
              }
            >
              <SelectTrigger className="w-full">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectGroup>
                  <SelectItem value="allow">allow</SelectItem>
                  <SelectItem value="deny">deny</SelectItem>
                </SelectGroup>
              </SelectContent>
            </Select>
          </Field>
          <Field>
            <FieldLabel>Tool allow/deny</FieldLabel>
            <Select
              items={[
                { label: "allow", value: "allow" },
                { label: "deny", value: "deny" },
              ]}
              value={policy.toolRule}
              onValueChange={(value) =>
                value && select("toolRule", value as McpPolicy["toolRule"])
              }
            >
              <SelectTrigger className="w-full">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectGroup>
                  <SelectItem value="allow">allow</SelectItem>
                  <SelectItem value="deny">deny</SelectItem>
                </SelectGroup>
              </SelectContent>
            </Select>
          </Field>
          <Field>
            <FieldLabel>Action per tool</FieldLabel>
            <Select
              items={["allow", "warn", "approval", "block"].map((v) => ({
                label: v,
                value: v,
              }))}
              value={policy.action}
              onValueChange={(value) =>
                value && select("action", value as McpPolicy["action"])
              }
            >
              <SelectTrigger className="w-full">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectGroup>
                  {["allow", "warn", "approval", "block"].map((v) => (
                    <SelectItem key={v} value={v}>
                      {v}
                    </SelectItem>
                  ))}
                </SelectGroup>
              </SelectContent>
            </Select>
          </Field>
          <Field>
            <FieldLabel>Risk override</FieldLabel>
            <Select
              items={["inherit", "low", "medium", "high"].map((v) => ({
                label: v,
                value: v,
              }))}
              value={policy.riskOverride}
              onValueChange={(value) =>
                value &&
                select("riskOverride", value as McpPolicy["riskOverride"])
              }
            >
              <SelectTrigger className="w-full">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectGroup>
                  {["inherit", "low", "medium", "high"].map((v) => (
                    <SelectItem key={v} value={v}>
                      {v}
                    </SelectItem>
                  ))}
                </SelectGroup>
              </SelectContent>
            </Select>
          </Field>
        </div>
        <Field>
          <FieldLabel htmlFor={`arguments-${policy.id}`}>
            Argument restrictions
          </FieldLabel>
          <Textarea
            id={`arguments-${policy.id}`}
            value={policy.argumentRule}
            onChange={(event) => select("argumentRule", event.target.value)}
          />
        </Field>
        <div className="grid gap-3 sm:grid-cols-3">
          {(
            [
              ["rpm", "RPM"],
              ["daily", "Per day"],
              ["concurrency", "Concurrency"],
              ["maxBytes", "Max bytes"],
              ["timeoutMs", "Timeout ms"],
            ] as const
          ).map(([key, label]) => (
            <Field key={key}>
              <FieldLabel htmlFor={`${key}-${policy.id}`}>{label}</FieldLabel>
              <Input
                id={`${key}-${policy.id}`}
                type="number"
                min="1"
                value={policy[key]}
                onChange={(event) => select(key, Number(event.target.value))}
              />
            </Field>
          ))}
        </div>
      </FieldGroup>
      <DialogFooter className="mt-4">
        <DialogClose render={<Button variant="outline" />}>Cancel</DialogClose>
        <DialogClose render={<Button type="submit" />}>
          Save simulated policy
        </DialogClose>
      </DialogFooter>
    </form>
  )
}

export function McpPoliciesPage() {
  const { state, dispatch } = useMockGateway()
  const policies = Object.values(state.policies).filter(
    (policy) => policy.tenantId === state.principal?.tenantId
  )
  return (
    <>
      <PageHeader
        title="MCP policies"
        description="Simulated scope, allow/deny, risk, approval, argument, rate, size, concurrency, and timeout controls."
        action={
          <Dialog>
            <DialogTrigger render={<Button />}>
              <PlusIcon data-icon="inline-start" />
              Create mock policy
            </DialogTrigger>
            <DialogContent className="max-h-[90vh] overflow-y-auto">
              <DialogHeader>
                <DialogTitle>Create MCP policy</DialogTitle>
                <DialogDescription>
                  Policy evaluation remains backend-owned.
                </DialogDescription>
              </DialogHeader>
              <PolicyForm value={defaultPolicy(state.principal!.tenantId)} />
            </DialogContent>
          </Dialog>
        }
      />
      <div className="grid gap-4 lg:grid-cols-2">
        {policies.map((policy) => (
          <Card key={policy.id}>
            <CardHeader>
              <CardTitle>{policy.name}</CardTitle>
              <CardDescription>
                {policy.projectId ?? "tenant-wide"} · {policy.action}
              </CardDescription>
              <CardAction>
                <Badge variant="outline">{policy.riskOverride} risk</Badge>
              </CardAction>
            </CardHeader>
            <CardContent className="flex flex-col gap-3">
              <p>{policy.argumentRule}</p>
              <p className="text-muted-foreground">
                {policy.rpm} RPM · {policy.daily}/day · {policy.concurrency}{" "}
                concurrent · {policy.timeoutMs}ms
              </p>
              <div className="flex gap-2">
                <Dialog>
                  <DialogTrigger
                    render={<Button size="sm" variant="outline" />}
                  >
                    Edit mock policy
                  </DialogTrigger>
                  <DialogContent className="max-h-[90vh] overflow-y-auto">
                    <DialogHeader>
                      <DialogTitle>Edit {policy.name}</DialogTitle>
                      <DialogDescription>
                        Simulated in-memory mutation.
                      </DialogDescription>
                    </DialogHeader>
                    <PolicyForm value={policy} />
                  </DialogContent>
                </Dialog>
                <Button
                  size="sm"
                  variant="destructive"
                  onClick={() =>
                    dispatch({ type: "policy.delete", id: policy.id })
                  }
                >
                  Delete mock policy
                </Button>
              </div>
            </CardContent>
          </Card>
        ))}
      </div>
    </>
  )
}

export function McpExplorerPage() {
  const { state, dispatch } = useMockGateway()
  const tools = Object.values(state.tools).filter(
    (tool) =>
      state.servers[tool.serverId]?.tenantId === state.principal?.tenantId &&
      state.servers[tool.serverId]?.enabled
  )
  const [tool, setTool] = React.useState(tools[0]?.name ?? "")
  const [args, setArgs] = React.useState('{"query":"quarterly report"}')
  const [scenario, setScenario] = React.useState<Invocation["scenario"]>("safe")
  const [key, setKey] = React.useState("idem-demo-001")
  const [error, setError] = React.useState("")
  const invocation = state.invocations[key]
  const activeTool = tools.some((item) => item.name === tool)
    ? tool
    : (tools[0]?.name ?? "")
  const run = () => {
    try {
      JSON.parse(args)
      setError("")
      dispatch({
        type: "invoke",
        tool: activeTool,
        scenario,
        idempotencyKey: key,
      })
    } catch {
      setError("Arguments must be valid JSON.")
    }
  }
  return (
    <>
      <PageHeader
        title="MCP explorer"
        description="Permitted servers and tools, JSON arguments, safe rendering, approval polling, and idempotent retry."
      />
      <BackendNotice />
      <div className="mt-4 grid gap-4 xl:grid-cols-[380px_minmax(0,1fr)]">
        <Card>
          <CardHeader>
            <CardTitle>Simulated invocation</CardTitle>
            <CardDescription>
              Nothing is sent to /v1/mcp/tools/call.
            </CardDescription>
          </CardHeader>
          <CardContent>
            <FieldGroup>
              <Field>
                <FieldLabel>Tool</FieldLabel>
                <Select
                  items={tools.map((item) => ({
                    label: item.name,
                    value: item.name,
                  }))}
                  value={activeTool}
                  onValueChange={(value) => value && setTool(value)}
                >
                  <SelectTrigger className="w-full">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectGroup>
                      {tools.map((item) => (
                        <SelectItem key={item.id} value={item.name}>
                          {item.name}
                        </SelectItem>
                      ))}
                    </SelectGroup>
                  </SelectContent>
                </Select>
              </Field>
              <Field data-invalid={Boolean(error)}>
                <FieldLabel htmlFor="mcp-args">JSON arguments</FieldLabel>
                <Textarea
                  id="mcp-args"
                  aria-invalid={Boolean(error)}
                  value={args}
                  onChange={(event) => setArgs(event.target.value)}
                />
                <FieldDescription>
                  {error ||
                    "Validated as JSON only; backend schema remains authoritative."}
                </FieldDescription>
              </Field>
              <Field>
                <FieldLabel>Security scenario</FieldLabel>
                <Select
                  items={[
                    "safe",
                    "warn",
                    "redact",
                    "block",
                    "approval",
                    "malicious-result",
                  ].map((v) => ({ label: v, value: v }))}
                  value={scenario}
                  onValueChange={(value) =>
                    value && setScenario(value as Invocation["scenario"])
                  }
                >
                  <SelectTrigger className="w-full">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectGroup>
                      {[
                        "safe",
                        "warn",
                        "redact",
                        "block",
                        "approval",
                        "malicious-result",
                      ].map((v) => (
                        <SelectItem key={v} value={v}>
                          {v}
                        </SelectItem>
                      ))}
                    </SelectGroup>
                  </SelectContent>
                </Select>
              </Field>
              <Field>
                <FieldLabel htmlFor="idempotency">
                  Mock idempotency key
                </FieldLabel>
                <Input
                  id="idempotency"
                  value={key}
                  onChange={(event) => setKey(event.target.value)}
                />
              </Field>
              <Button onClick={run}>Invoke / retry simulation</Button>
            </FieldGroup>
          </CardContent>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle>Safe result renderer</CardTitle>
            <CardDescription>
              Text only; tool-provided HTML is never executed.
            </CardDescription>
            <CardAction>
              {invocation && <StatusBadge status={invocation.status} />}
            </CardAction>
          </CardHeader>
          <CardContent className="flex flex-col gap-4">
            {invocation ? (
              <>
                <pre className="rounded-md bg-muted p-4 text-xs whitespace-pre-wrap">
                  {invocation.result ??
                    `Approval required: ${invocation.approvalId}`}
                </pre>
                <div className="flex flex-wrap gap-2">
                  <Badge variant="outline">
                    idempotency: {invocation.idempotencyKey}
                  </Badge>
                  <Badge variant="outline">result count: 1</Badge>
                  {invocation.approvalId && (
                    <Button size="sm" variant="outline" onClick={run}>
                      <ArrowsClockwiseIcon data-icon="inline-start" />
                      Poll approval & retry
                    </Button>
                  )}
                </div>
              </>
            ) : (
              <Alert>
                <CheckCircleIcon />
                <AlertTitle>Ready</AlertTitle>
                <AlertDescription>
                  Choose a deterministic scenario and invoke.
                </AlertDescription>
              </Alert>
            )}
          </CardContent>
        </Card>
      </div>
    </>
  )
}

export function ApprovalsPage() {
  const { state, dispatch } = useMockGateway()
  const [filter, setFilter] = React.useState("pending")
  const approvals = visibleApprovals(state).filter(
    (item) => filter === "all" || item.status === filter
  )
  return (
    <>
      <PageHeader
        title="Approval inbox"
        description="Tenant-isolated sanitized approval details, expiry, approve, and reject simulations."
      />
      <Tabs value={filter} onValueChange={setFilter}>
        <TabsList>
          {["pending", "approved", "rejected", "expired", "all"].map(
            (value) => (
              <TabsTrigger key={value} value={value}>
                {value}
              </TabsTrigger>
            )
          )}
        </TabsList>
        <TabsContent value={filter} className="pt-3">
          <Card>
            <CardHeader>
              <CardTitle>{filter} approvals</CardTitle>
              <CardDescription>
                Selected tenant only; no caller-wide history API exists yet.
              </CardDescription>
            </CardHeader>
            <CardContent>
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>ID</TableHead>
                    <TableHead>Tool</TableHead>
                    <TableHead>Sanitized detail</TableHead>
                    <TableHead>Expiry</TableHead>
                    <TableHead>Status</TableHead>
                    <TableHead>Decision</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {approvals.map((approval) => (
                    <TableRow key={approval.id}>
                      <TableCell className="font-mono">{approval.id}</TableCell>
                      <TableCell>{approval.tool}</TableCell>
                      <TableCell>{approval.summary}</TableCell>
                      <TableCell>{approval.expiresAt}</TableCell>
                      <TableCell>
                        <StatusBadge status={approval.status} />
                      </TableCell>
                      <TableCell>
                        {approval.status === "pending" && (
                          <div className="flex gap-2">
                            <Button
                              size="sm"
                              onClick={() =>
                                dispatch({
                                  type: "approval.decide",
                                  id: approval.id,
                                  status: "approved",
                                  reason: "Approved in simulated inbox",
                                })
                              }
                            >
                              Approve mock
                            </Button>
                            <Button
                              size="sm"
                              variant="destructive"
                              onClick={() =>
                                dispatch({
                                  type: "approval.decide",
                                  id: approval.id,
                                  status: "rejected",
                                  reason: "Rejected in simulated inbox",
                                })
                              }
                            >
                              Reject mock
                            </Button>
                            <Button
                              size="sm"
                              variant="outline"
                              onClick={() =>
                                dispatch({
                                  type: "approval.expire",
                                  id: approval.id,
                                })
                              }
                            >
                              Expire mock
                            </Button>
                          </div>
                        )}
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>
    </>
  )
}
