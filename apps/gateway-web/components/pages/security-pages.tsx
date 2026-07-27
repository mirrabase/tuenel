"use client"

import * as React from "react"
import { CheckIcon } from "@phosphor-icons/react"
import { toast } from "sonner"

import { useGateway } from "@/components/gateway-provider"
import {
  DataState,
  PageHeader,
  StatusBadge,
  useGatewayData,
} from "@/components/pages/shared"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { Textarea } from "@/components/ui/textarea"
import { type Page, gatewayFetch } from "@/lib/gateway-api"

type SecurityRecord = Record<string, unknown> & {
  policy_id?: string
  pattern_id?: string
  incident_id?: string
  finding_id?: string
  event_id?: string
  status?: string
}

export function SecurityPoliciesPage() {
  return (
    <>
      <PageHeader title="Security policies" />
      <Tabs defaultValue="policies">
        <TabsList>
          <TabsTrigger value="policies">Policies</TabsTrigger>
          <TabsTrigger value="patterns">Custom patterns</TabsTrigger>
        </TabsList>
        <TabsContent value="policies" className="pt-4">
          <SecurityResource
            path="/admin/security/policies"
            idField="policy_id"
            initial={{
              name: "",
              enabled: true,
              scope_kind: "tenant",
              scope_id: "",
              policy: {
                minimum_action_by_severity: {},
                category_actions: {},
                fail_open: false,
              },
            }}
          />
        </TabsContent>
        <TabsContent value="patterns" className="pt-4">
          <SecurityResource
            path="/admin/security/patterns"
            idField="pattern_id"
            initial={{
              name: "",
              category: "policy_violation",
              pattern: "",
              enabled: true,
            }}
          />
        </TabsContent>
      </Tabs>
    </>
  )
}

export function SecurityOperationsPage() {
  const { tenantId } = useGateway()
  const incidents = useGatewayData<Page<SecurityRecord>>(
    "/admin/security/incidents"
  )
  const findings = useGatewayData<Page<SecurityRecord>>(
    "/admin/security/findings"
  )
  const events = useGatewayData<Page<SecurityRecord>>("/admin/security/events")
  return (
    <>
      <PageHeader title="Security operations" />
      <Tabs defaultValue="incidents">
        <TabsList>
          <TabsTrigger value="incidents">Incidents</TabsTrigger>
          <TabsTrigger value="findings">Findings</TabsTrigger>
          <TabsTrigger value="events">Events</TabsTrigger>
        </TabsList>
        <TabsContent value="incidents" className="pt-4">
          <SecurityList
            state={incidents}
            action={async (record) => {
              await gatewayFetch(
                `/admin/security/incidents/${record.incident_id}`,
                tenantId,
                {
                  method: "PATCH",
                  headers: { "content-type": "application/json" },
                  body: JSON.stringify({ status: "resolved" }),
                }
              )
              incidents.reload()
            }}
          />
        </TabsContent>
        <TabsContent value="findings" className="pt-4">
          <SecurityList state={findings} />
        </TabsContent>
        <TabsContent value="events" className="pt-4">
          <SecurityList state={events} />
        </TabsContent>
      </Tabs>
    </>
  )
}

function SecurityResource({
  path,
  idField,
  initial,
}: {
  path: string
  idField: string
  initial: Record<string, unknown>
}) {
  const { tenantId } = useGateway()
  const state = useGatewayData<Page<SecurityRecord>>(path)
  const [body, setBody] = React.useState(JSON.stringify(initial, null, 2))
  return (
    <div className="flex flex-col gap-4">
      <Card>
        <CardHeader>
          <CardTitle>Create resource</CardTitle>
        </CardHeader>
        <CardContent>
          <FieldGroup>
            <Field>
              <FieldLabel htmlFor={`${idField}-body`}>JSON</FieldLabel>
              <Textarea
                id={`${idField}-body`}
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
      <SecurityList state={state} />
    </div>
  )
}

function SecurityList({
  state,
  action,
}: {
  state: ReturnType<typeof useGatewayData<Page<SecurityRecord>>>
  action?: (record: SecurityRecord) => Promise<void>
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
        <CardContent className="flex flex-col gap-3">
          {records.map((record, index) => (
            <div
              key={String(
                record.incident_id ??
                  record.finding_id ??
                  record.event_id ??
                  record.policy_id ??
                  record.pattern_id ??
                  index
              )}
              className="flex items-start justify-between gap-3 rounded-md border p-3"
            >
              <pre className="overflow-auto text-xs whitespace-pre-wrap">
                {JSON.stringify(record, null, 2)}
              </pre>
              <div className="flex flex-col items-end gap-2">
                {record.status && <StatusBadge status={record.status} />}
                {action && record.status !== "resolved" && (
                  <Button
                    size="sm"
                    variant="outline"
                    onClick={async () => {
                      try {
                        await action(record)
                      } catch (error) {
                        toast.error(
                          error instanceof Error
                            ? error.message
                            : "Update failed"
                        )
                      }
                    }}
                  >
                    <CheckIcon data-icon="inline-start" />
                    Resolve
                  </Button>
                )}
              </div>
            </div>
          ))}
        </CardContent>
      </Card>
    </DataState>
  )
}
