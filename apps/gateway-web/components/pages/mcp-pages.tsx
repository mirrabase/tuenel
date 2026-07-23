"use client"

import * as React from "react"
import {
  ArrowClockwiseIcon,
  CheckIcon,
  PlayIcon,
  TrashIcon,
  XIcon,
} from "@phosphor-icons/react"
import { toast } from "sonner"

import { useGateway } from "@/components/gateway-provider"
import {
  DataState,
  PageHeader,
  StatusBadge,
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
import { Textarea } from "@/components/ui/textarea"
import { type Page, gatewayFetch } from "@/lib/gateway-api"

type McpRecord = Record<string, unknown> & {
  server_id?: string
  policy_id?: string
  approval_id?: string
  tool_name?: string
  name?: string
  status?: string
  enabled?: boolean
}

export function McpRegistryPage() {
  const { tenantId } = useGateway()
  const state = useGatewayData<Page<McpRecord>>("/admin/mcp/servers")
  const [name, setName] = React.useState("")
  const [endpoint, setEndpoint] = React.useState("")
  const [credential, setCredential] = React.useState("")

  async function action(path: string, method = "POST") {
    try {
      await gatewayFetch(path, tenantId, { method })
      state.reload()
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Request failed")
    }
  }

  return (
    <>
      <PageHeader
        title="MCP registry"
        description="Durable MCP servers, discovery, and health checks."
      />
      <Card className="mb-4">
        <CardHeader>
          <CardTitle>Register HTTP server</CardTitle>
          <CardDescription>
            The credential is encrypted and never returned.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <form
            onSubmit={async (event) => {
              event.preventDefault()
              try {
                await gatewayFetch("/admin/mcp/servers", tenantId, {
                  method: "POST",
                  headers: { "content-type": "application/json" },
                  body: JSON.stringify({
                    name,
                    transport_type: "streamable_http",
                    endpoint,
                    credential: credential || undefined,
                    enabled: true,
                  }),
                })
                setName("")
                setEndpoint("")
                setCredential("")
                state.reload()
              } catch (error) {
                toast.error(
                  error instanceof Error ? error.message : "Create failed"
                )
              }
            }}
          >
            <FieldGroup>
              <Field>
                <FieldLabel htmlFor="mcp-name">Name</FieldLabel>
                <Input
                  id="mcp-name"
                  value={name}
                  onChange={(event) => setName(event.target.value)}
                  required
                />
              </Field>
              <Field>
                <FieldLabel htmlFor="mcp-endpoint">Endpoint</FieldLabel>
                <Input
                  id="mcp-endpoint"
                  type="url"
                  value={endpoint}
                  onChange={(event) => setEndpoint(event.target.value)}
                  required
                />
              </Field>
              <Field>
                <FieldLabel htmlFor="mcp-credential">Credential</FieldLabel>
                <Input
                  id="mcp-credential"
                  type="password"
                  autoComplete="off"
                  value={credential}
                  onChange={(event) => setCredential(event.target.value)}
                />
              </Field>
              <Button type="submit">Register</Button>
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
                  <TableHead>Server</TableHead>
                  <TableHead>Status</TableHead>
                  <TableHead>Actions</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {state.data?.data.map((server) => (
                  <TableRow key={server.server_id}>
                    <TableCell>
                      <pre className="text-xs whitespace-pre-wrap">
                        {JSON.stringify(server, null, 2)}
                      </pre>
                    </TableCell>
                    <TableCell>
                      <StatusBadge
                        status={server.enabled ? "Active" : "Disabled"}
                      />
                    </TableCell>
                    <TableCell>
                      <div className="flex flex-wrap gap-2">
                        <Button
                          size="sm"
                          variant="outline"
                          onClick={() =>
                            action(
                              `/admin/mcp/servers/${server.server_id}/health`
                            )
                          }
                        >
                          <CheckIcon data-icon="inline-start" />
                          Check
                        </Button>
                        <Button
                          size="sm"
                          variant="outline"
                          onClick={() =>
                            action(
                              `/admin/mcp/servers/${server.server_id}/refresh`
                            )
                          }
                        >
                          <ArrowClockwiseIcon data-icon="inline-start" />
                          Refresh
                        </Button>
                        <Button
                          size="sm"
                          variant="outline"
                          onClick={() => {
                            if (window.confirm(`Retire ${server.name}?`))
                              void action(
                                `/admin/mcp/servers/${server.server_id}`,
                                "DELETE"
                              )
                          }}
                        >
                          <TrashIcon data-icon="inline-start" />
                          Retire
                        </Button>
                      </div>
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

export function McpPoliciesPage() {
  return (
    <JsonResourcePage
      title="MCP policies"
      description="Policy precedence and tool constraints enforced by the MCP pipeline."
      path="/admin/mcp/policies"
      idField="policy_id"
      initial={{
        name: "",
        scope_kind: "tenant",
        scope_id: "",
        policy: {
          allowed_servers: [],
          denied_servers: [],
          allowed_tools: [],
          denied_tools: [],
          require_approval_for: [],
        },
      }}
    />
  )
}

export function McpExplorerPage() {
  const { tenantId } = useGateway()
  const tools = useGatewayData<Page<McpRecord>>("/v1/mcp/tools")
  const [argumentsJson, setArgumentsJson] = React.useState("{}")
  const [result, setResult] = React.useState<unknown>()

  return (
    <>
      <PageHeader
        title="MCP explorer"
        description="Invoke policy-filtered tools through the real MCP pipeline."
      />
      <DataState
        loading={tools.loading}
        error={tools.error}
        empty={tools.data?.data.length === 0}
        onRetry={tools.reload}
      >
        <div className="grid gap-4 lg:grid-cols-2">
          <Card>
            <CardHeader>
              <CardTitle>Available tools</CardTitle>
            </CardHeader>
            <CardContent className="flex flex-col gap-3">
              <Field>
                <FieldLabel htmlFor="mcp-arguments">Arguments JSON</FieldLabel>
                <Textarea
                  id="mcp-arguments"
                  value={argumentsJson}
                  onChange={(event) => setArgumentsJson(event.target.value)}
                />
              </Field>
              {tools.data?.data.map((tool) => (
                <div
                  key={`${tool.server_id}:${tool.tool_name}`}
                  className="flex items-start justify-between gap-3 rounded-md border p-3"
                >
                  <pre className="overflow-auto text-xs whitespace-pre-wrap">
                    {JSON.stringify(tool, null, 2)}
                  </pre>
                  <Button
                    size="sm"
                    onClick={async () => {
                      try {
                        const value = await gatewayFetch(
                          "/v1/mcp/tools/call",
                          tenantId,
                          {
                            method: "POST",
                            headers: {
                              "content-type": "application/json",
                              "idempotency-key": crypto.randomUUID(),
                            },
                            body: JSON.stringify({
                              server_id: tool.server_id,
                              tool_name: tool.tool_name,
                              arguments: JSON.parse(argumentsJson),
                            }),
                          }
                        )
                        setResult(value)
                      } catch (error) {
                        toast.error(
                          error instanceof Error
                            ? error.message
                            : "Invocation failed"
                        )
                      }
                    }}
                  >
                    <PlayIcon data-icon="inline-start" />
                    Invoke
                  </Button>
                </div>
              ))}
            </CardContent>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>Result</CardTitle>
              <CardDescription>Sanitized gateway response.</CardDescription>
            </CardHeader>
            <CardContent>
              <pre className="overflow-auto text-xs whitespace-pre-wrap">
                {result
                  ? JSON.stringify(result, null, 2)
                  : "No invocation yet."}
              </pre>
            </CardContent>
          </Card>
        </div>
      </DataState>
    </>
  )
}

export function ApprovalsPage() {
  const { tenantId } = useGateway()
  const state = useGatewayData<Page<McpRecord>>("/admin/approvals")
  async function decide(id: string, decision: "approve" | "reject") {
    try {
      await gatewayFetch(`/admin/approvals/${id}/${decision}`, tenantId, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({}),
      })
      state.reload()
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Decision failed")
    }
  }
  return (
    <>
      <PageHeader
        title="Approval inbox"
        description="Tenant-scoped human decisions for protected operations."
      />
      <DataState
        loading={state.loading}
        error={state.error}
        empty={state.data?.data.length === 0}
        onRetry={state.reload}
      >
        <Card>
          <CardContent className="flex flex-col gap-3">
            {state.data?.data.map((approval) => (
              <div
                key={approval.approval_id}
                className="flex items-start justify-between gap-3 rounded-md border p-3"
              >
                <pre className="overflow-auto text-xs whitespace-pre-wrap">
                  {JSON.stringify(approval, null, 2)}
                </pre>
                {approval.status === "pending" && (
                  <div className="flex gap-2">
                    <Button
                      size="sm"
                      onClick={() => decide(approval.approval_id!, "approve")}
                    >
                      <CheckIcon data-icon="inline-start" />
                      Approve
                    </Button>
                    <Button
                      size="sm"
                      variant="outline"
                      onClick={() => decide(approval.approval_id!, "reject")}
                    >
                      <XIcon data-icon="inline-start" />
                      Reject
                    </Button>
                  </div>
                )}
              </div>
            ))}
          </CardContent>
        </Card>
      </DataState>
    </>
  )
}

function JsonResourcePage({
  title,
  description,
  path,
  idField,
  initial,
}: {
  title: string
  description: string
  path: string
  idField: string
  initial: Record<string, unknown>
}) {
  const { tenantId } = useGateway()
  const state = useGatewayData<Page<McpRecord>>(path)
  const [body, setBody] = React.useState(JSON.stringify(initial, null, 2))
  return (
    <>
      <PageHeader title={title} description={description} />
      <Card className="mb-4">
        <CardHeader>
          <CardTitle>Create</CardTitle>
        </CardHeader>
        <CardContent>
          <FieldGroup>
            <Field>
              <FieldLabel htmlFor={`${idField}-json`}>JSON</FieldLabel>
              <Textarea
                id={`${idField}-json`}
                value={body}
                onChange={(event) => setBody(event.target.value)}
                rows={10}
              />
            </Field>
            <Button
              onClick={async () => {
                try {
                  await gatewayFetch(path, tenantId, {
                    method: "POST",
                    headers: { "content-type": "application/json" },
                    body: JSON.stringify(JSON.parse(body)),
                  })
                  state.reload()
                } catch (error) {
                  toast.error(
                    error instanceof Error ? error.message : "Create failed"
                  )
                }
              }}
            >
              Save
            </Button>
          </FieldGroup>
        </CardContent>
      </Card>
      <DataState
        loading={state.loading}
        error={state.error}
        empty={state.data?.data.length === 0}
        onRetry={state.reload}
      >
        <Card>
          <CardContent className="flex flex-col gap-3">
            {state.data?.data.map((record) => (
              <pre
                key={String(record[idField])}
                className="overflow-auto rounded-md border p-3 text-xs whitespace-pre-wrap"
              >
                {JSON.stringify(record, null, 2)}
              </pre>
            ))}
          </CardContent>
        </Card>
      </DataState>
    </>
  )
}
