"use client"

import * as React from "react"
import {
  EyeIcon,
  ShieldWarningIcon,
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
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import {
  Field,
  FieldDescription,
  FieldGroup,
  FieldLabel,
  FieldLegend,
  FieldSet,
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
import { Textarea } from "@/components/ui/textarea"
import type { SecurityAction } from "@/lib/mock-store"

export function SecurityPoliciesPage() {
  const { state, dispatch } = useMockGateway()
  const original = Object.values(state.securityPolicies)[0]
  const [policy, setPolicy] = React.useState(original)
  const toggle = (
    key:
      | "inspectRequest"
      | "inspectResponse"
      | "inspectMcpArguments"
      | "inspectMcpResult"
      | "failOpen"
      | "createIncident",
    value: boolean
  ) => setPolicy((current) => ({ ...current, [key]: value }))
  return (
    <>
      <PageHeader
        title="Security policies"
        description="Simulated inspection stages, incident behavior, content ceiling, and category/severity actions."
      />
      <BackendNotice>
        Detector and policy business logic stays in the backend. This editor
        selects deterministic fixtures only.
      </BackendNotice>
      {policy.failOpen && (
        <Alert className="mt-4" variant="destructive">
          <WarningCircleIcon />
          <AlertTitle>Fail-open enabled</AlertTitle>
          <AlertDescription>
            Requests would continue when inspection is unavailable. Use only
            with an explicit risk decision.
          </AlertDescription>
        </Alert>
      )}
      <Card className="mt-4">
        <CardHeader>
          <CardTitle>{policy.name}</CardTitle>
          <CardDescription>Mock policy editor</CardDescription>
        </CardHeader>
        <CardContent>
          <FieldGroup>
            <FieldSet>
              <FieldLegend variant="label">Inspection stages</FieldLegend>
              <FieldDescription>
                Enable the stages represented by the Gateway v0.3 policy
                contract.
              </FieldDescription>
              <FieldGroup className="gap-3">
                {(
                  [
                    ["inspectRequest", "Request"],
                    ["inspectResponse", "Response"],
                    ["inspectMcpArguments", "MCP arguments"],
                    ["inspectMcpResult", "MCP result"],
                    ["createIncident", "Create incident"],
                    ["failOpen", "Fail open"],
                  ] as const
                ).map(([key, label]) => (
                  <Field orientation="horizontal" key={key}>
                    <FieldLabel htmlFor={key}>{label}</FieldLabel>
                    <Switch
                      id={key}
                      checked={policy[key]}
                      onCheckedChange={(value) => toggle(key, value)}
                    />
                  </Field>
                ))}
              </FieldGroup>
            </FieldSet>
            <Field>
              <FieldLabel htmlFor="max-content">
                Maximum content size (bytes)
              </FieldLabel>
              <Input
                id="max-content"
                type="number"
                min="1"
                value={policy.maxBytes}
                onChange={(event) =>
                  setPolicy((current) => ({
                    ...current,
                    maxBytes: Number(event.target.value),
                  }))
                }
              />
            </Field>
            <FieldSet>
              <FieldLegend variant="label">
                Category / severity action matrix
              </FieldLegend>
              <FieldGroup>
                {(
                  Object.keys(policy.matrix) as Array<
                    keyof typeof policy.matrix
                  >
                ).map((category) => (
                  <Field orientation="horizontal" key={category}>
                    <FieldLabel>
                      {category} /{" "}
                      {category === "injection" || category === "malware"
                        ? "critical"
                        : category === "credentials"
                          ? "high"
                          : "medium"}
                    </FieldLabel>
                    <Select
                      items={["allow", "warn", "redact", "block"].map(
                        (value) => ({ label: value, value })
                      )}
                      value={policy.matrix[category]}
                      onValueChange={(value) =>
                        value &&
                        setPolicy((current) => ({
                          ...current,
                          matrix: {
                            ...current.matrix,
                            [category]: value as SecurityAction,
                          },
                        }))
                      }
                    >
                      <SelectTrigger>
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectGroup>
                          {["allow", "warn", "redact", "block"].map((value) => (
                            <SelectItem key={value} value={value}>
                              {value}
                            </SelectItem>
                          ))}
                        </SelectGroup>
                      </SelectContent>
                    </Select>
                  </Field>
                ))}
              </FieldGroup>
            </FieldSet>
            <Button
              onClick={() => dispatch({ type: "security-policy.save", policy })}
            >
              Save simulated policy
            </Button>
          </FieldGroup>
        </CardContent>
      </Card>
    </>
  )
}

export function SecurityOperationsPage() {
  const { state, dispatch } = useMockGateway()
  const tenantId = state.principal?.tenantId
  const [status, setStatus] = React.useState("all")
  const incidents = Object.values(state.incidents).filter(
    (item) =>
      item.tenantId === tenantId && (status === "all" || item.status === status)
  )
  const findings = state.findings.filter((item) => item.tenantId === tenantId)
  const events = state.securityEvents.filter(
    (item) => item.tenantId === tenantId
  )
  return (
    <>
      <PageHeader
        title="Security operations"
        description="Tenant-isolated incidents, findings, events, risk scores, and sanitized request references."
      />
      <Tabs defaultValue="incidents">
        <TabsList>
          <TabsTrigger value="incidents">Incidents</TabsTrigger>
          <TabsTrigger value="findings">Findings</TabsTrigger>
          <TabsTrigger value="events">Events</TabsTrigger>
        </TabsList>
        <TabsContent value="incidents" className="pt-3">
          <Card>
            <CardHeader>
              <CardTitle>Incident queue</CardTitle>
              <CardDescription>
                Notes accept sanitized text only.
              </CardDescription>
            </CardHeader>
            <CardContent className="flex flex-col gap-3">
              <Select
                items={["all", "open", "investigating", "resolved"].map(
                  (value) => ({ label: value, value })
                )}
                value={status}
                onValueChange={(value) => value && setStatus(value)}
              >
                <SelectTrigger className="w-48">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectGroup>
                    {["all", "open", "investigating", "resolved"].map(
                      (value) => (
                        <SelectItem key={value} value={value}>
                          {value}
                        </SelectItem>
                      )
                    )}
                  </SelectGroup>
                </SelectContent>
              </Select>
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Incident</TableHead>
                    <TableHead>Request</TableHead>
                    <TableHead>Risk</TableHead>
                    <TableHead>Status</TableHead>
                    <TableHead>Detail</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {incidents.map((incident) => (
                    <TableRow key={incident.id}>
                      <TableCell>
                        <div className="font-medium">{incident.title}</div>
                        <div className="font-mono text-muted-foreground">
                          {incident.id}
                        </div>
                      </TableCell>
                      <TableCell className="font-mono">
                        {incident.requestId}
                      </TableCell>
                      <TableCell>
                        <Badge variant="outline">
                          {incident.riskScore}/100 · {incident.severity}
                        </Badge>
                      </TableCell>
                      <TableCell>
                        <StatusBadge status={incident.status} />
                      </TableCell>
                      <TableCell>
                        <Sheet>
                          <SheetTrigger
                            render={<Button size="sm" variant="outline" />}
                          >
                            <EyeIcon data-icon="inline-start" />
                            Inspect
                          </SheetTrigger>
                          <SheetContent>
                            <SheetHeader>
                              <SheetTitle>{incident.title}</SheetTitle>
                              <SheetDescription>
                                Sanitized operational detail for{" "}
                                {incident.requestId}.
                              </SheetDescription>
                            </SheetHeader>
                            <IncidentEditor
                              incident={incident}
                              onSave={(nextStatus, note) =>
                                dispatch({
                                  type: "incident.status",
                                  id: incident.id,
                                  status: nextStatus,
                                  note,
                                })
                              }
                            />
                          </SheetContent>
                        </Sheet>
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </CardContent>
          </Card>
        </TabsContent>
        <TabsContent value="findings" className="pt-3">
          <Card>
            <CardHeader>
              <CardTitle>Sanitized findings</CardTitle>
              <CardDescription>
                Evidence never includes complete secrets or raw malicious
                content.
              </CardDescription>
            </CardHeader>
            <CardContent>
              {findings.map((finding) => (
                <Alert key={finding.id} className="mb-3">
                  <ShieldWarningIcon />
                  <AlertTitle>
                    {finding.category} · {finding.severity}
                  </AlertTitle>
                  <AlertDescription>{finding.evidence}</AlertDescription>
                </Alert>
              ))}
            </CardContent>
          </Card>
        </TabsContent>
        <TabsContent value="events" className="pt-3">
          <Card>
            <CardHeader>
              <CardTitle>Security events</CardTitle>
              <CardDescription>
                Mock references join back to gateway request IDs.
              </CardDescription>
            </CardHeader>
            <CardContent>
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Event</TableHead>
                    <TableHead>Request</TableHead>
                    <TableHead>Action</TableHead>
                    <TableHead>Detail</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {events.map((event) => (
                    <TableRow key={event.id}>
                      <TableCell className="font-mono">{event.id}</TableCell>
                      <TableCell className="font-mono">
                        {event.requestId}
                      </TableCell>
                      <TableCell>
                        <StatusBadge status={event.action} />
                      </TableCell>
                      <TableCell>{event.detail}</TableCell>
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

function IncidentEditor({
  incident,
  onSave,
}: {
  incident: { status: "open" | "investigating" | "resolved"; note: string }
  onSave: (status: "open" | "investigating" | "resolved", note: string) => void
}) {
  const [status, setStatus] = React.useState(incident.status)
  const [note, setNote] = React.useState(incident.note)
  return (
    <div className="flex flex-col gap-4 p-4">
      <FieldGroup>
        <Field>
          <FieldLabel>Status</FieldLabel>
          <Select
            items={["open", "investigating", "resolved"].map((value) => ({
              label: value,
              value,
            }))}
            value={status}
            onValueChange={(value) =>
              value && setStatus(value as typeof status)
            }
          >
            <SelectTrigger className="w-full">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectGroup>
                {["open", "investigating", "resolved"].map((value) => (
                  <SelectItem key={value} value={value}>
                    {value}
                  </SelectItem>
                ))}
              </SelectGroup>
            </SelectContent>
          </Select>
        </Field>
        <Field>
          <FieldLabel htmlFor="incident-note">Sanitized note</FieldLabel>
          <Textarea
            id="incident-note"
            value={note}
            onChange={(event) => setNote(event.target.value)}
          />
        </Field>
        <Button onClick={() => onSave(status, note)}>
          Update simulated incident
        </Button>
      </FieldGroup>
    </div>
  )
}
