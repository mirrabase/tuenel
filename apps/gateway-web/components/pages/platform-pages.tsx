"use client"

import * as React from "react"
import {
  ArrowClockwiseIcon,
  InfoIcon,
  PencilSimpleIcon,
  PlusIcon,
  TrashIcon,
} from "@phosphor-icons/react"
import { toast } from "sonner"

import { useGateway } from "@/components/gateway-provider"
import {
  DataState,
  PageHeader,
  StatusBadge,
  useGatewayData,
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
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
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
  FieldError,
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
import { Spinner } from "@/components/ui/spinner"
import { Switch } from "@/components/ui/switch"
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
import { buildResourcePayload } from "@/lib/platform-resources"

export type PlatformKind =
  | "tenants"
  | "projects"
  | "providers"
  | "routing"
  | "pricing"
  | "policies"
  | "quotas"
  | "ledger"
  | "system"
  | "integrations"

type ResourceKind =
  "projects" | "providers" | "routing" | "pricing" | "policies" | "quotas"

type RecordValue = Record<string, unknown> & {
  id?: string
  event_id?: string
  tenant_id?: string | null
  version?: number
  status?: string
  enabled?: boolean
}

type ResourceConfig = {
  kind?: ResourceKind
  title: string
  description: string
  path: string
  mutable: boolean
}

const config: Record<
  Exclude<PlatformKind, "system" | "integrations">,
  ResourceConfig
> = {
  tenants: {
    title: "Tenants",
    description: "Durable gateway tenants visible to this operator.",
    path: "/admin/tenants",
    mutable: false,
  },
  projects: {
    kind: "projects",
    title: "Projects",
    description: "Tenant projects used by policy and quota scopes.",
    path: "/admin/projects",
    mutable: true,
  },
  providers: {
    kind: "providers",
    title: "Providers",
    description: "Global provider adapters and write-only credentials.",
    path: "/admin/providers",
    mutable: true,
  },
  routing: {
    kind: "routing",
    title: "Model routing",
    description: "Global ordered provider targets used by live inference.",
    path: "/admin/model-routes",
    mutable: true,
  },
  pricing: {
    kind: "pricing",
    title: "Model pricing",
    description: "Global versioned prices used for usage cost estimation.",
    path: "/admin/model-prices",
    mutable: true,
  },
  policies: {
    kind: "policies",
    title: "General policies",
    description: "Tenant-scoped inference authorization policies.",
    path: "/admin/policies",
    mutable: true,
  },
  quotas: {
    kind: "quotas",
    title: "Quota limits",
    description: "Tenant-scoped token, cost, concurrency, and request limits.",
    path: "/admin/quota-limits",
    mutable: true,
  },
  ledger: {
    title: "Usage ledger",
    description: "Immutable usage events from PostgreSQL.",
    path: "/admin/usage/events",
    mutable: false,
  },
}

const globalKinds = new Set<ResourceKind>(["providers", "routing", "pricing"])

export function PlatformPage({ kind }: { kind: PlatformKind }) {
  if (kind === "system") return <SystemPage />
  if (kind === "integrations") return <IntegrationsPage />
  return <ResourcePage {...config[kind]} />
}

function ResourcePage({
  kind,
  title,
  description,
  path,
  mutable,
}: ResourceConfig) {
  const { tenantId, tenantRole, gatewayAdmin, projectId } = useGateway()
  const scopedPath = projectId
    ? `${path}?tenant_id=${tenantId}&project_id=${projectId}`
    : path
  const state = useGatewayData<Page<RecordValue>>(scopedPath)
  const [editor, setEditor] = React.useState<RecordValue | "new" | null>(null)
  const [retiring, setRetiring] = React.useState<RecordValue | null>(null)
  const [submitting, setSubmitting] = React.useState(false)
  const [formError, setFormError] = React.useState("")
  const global = kind ? globalKinds.has(kind) && !projectId : false
  const canWrite =
    Boolean(kind) &&
    (global
      ? gatewayAdmin
      : gatewayAdmin || ["owner", "admin"].includes(tenantRole))
  const records =
    state.data?.data.filter(
      (record) =>
        !kind ||
        (global ? record.tenant_id == null : record.tenant_id === tenantId)
    ) ?? []

  function closeEditor() {
    setEditor(null)
    setFormError("")
  }

  async function save(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (!kind || !editor) return
    setSubmitting(true)
    setFormError("")
    const editing = editor !== "new"
    try {
      const body = buildResourcePayload(
        kind,
        new FormData(event.currentTarget),
        tenantId,
        editing,
        projectId
      )
      const id = editing ? String(editor.id) : ""
      await gatewayFetch(editing ? `${path}/${id}` : path, tenantId, {
        method: editing ? "PATCH" : "POST",
        headers: {
          "content-type": "application/json",
          ...(editing ? { "if-match": `"${editor.version ?? 1}"` } : {}),
        },
        body: JSON.stringify(body),
      })
      closeEditor()
      state.reload()
      toast.success(`${title} resource ${editing ? "updated" : "created"}`)
    } catch (error) {
      const message = error instanceof Error ? error.message : "Save failed"
      setFormError(message)
      toast.error(message)
    } finally {
      setSubmitting(false)
    }
  }

  async function retire() {
    if (!retiring) return
    const id = String(retiring.id)
    setSubmitting(true)
    try {
      await gatewayFetch(`${path}/${id}`, tenantId, {
        method: "DELETE",
        headers: { "if-match": `"${retiring.version ?? 1}"` },
      })
      setRetiring(null)
      state.reload()
      toast.success(`${title} resource retired`)
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Retirement failed")
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <>
      <PageHeader
        title={title}
        description={description}
        action={
          mutable &&
          canWrite && (
            <Button onClick={() => setEditor("new")}>
              <PlusIcon data-icon="inline-start" />
              Add
            </Button>
          )
        }
      />
      {mutable && !canWrite && (
        <Alert className="mb-4">
          <InfoIcon />
          <AlertTitle>Read-only access</AlertTitle>
          <AlertDescription>
            {global
              ? "Global providers, routing, and pricing can only be changed by a gateway administrator."
              : "Only tenant owners and administrators can change this resource."}
          </AlertDescription>
        </Alert>
      )}
      {global && gatewayAdmin && (
        <Alert className="mb-4">
          <InfoIcon />
          <AlertTitle>Runtime configuration</AlertTitle>
          <AlertDescription>
            Changes are reconciled asynchronously. Check System if the runtime
            reports a configuration error.
          </AlertDescription>
        </Alert>
      )}
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
                  <TableHead>Resource</TableHead>
                  <TableHead>Status</TableHead>
                  {mutable && <TableHead />}
                </TableRow>
              </TableHeader>
              <TableBody>
                {records.map((record, index) => {
                  const id = String(record.id ?? record.event_id ?? index)
                  return (
                    <TableRow key={id}>
                      <TableCell>
                        <pre className="max-w-4xl overflow-auto text-xs whitespace-pre-wrap">
                          {JSON.stringify(record, null, 2)}
                        </pre>
                      </TableCell>
                      <TableCell>
                        <StatusBadge
                          status={
                            record.status ??
                            (record.enabled === false ? "Retired" : "Active")
                          }
                        />
                      </TableCell>
                      {mutable && (
                        <TableCell className="text-right">
                          <div className="flex justify-end gap-2">
                            <Button
                              size="sm"
                              variant="outline"
                              disabled={!canWrite}
                              onClick={() => setEditor(record)}
                            >
                              <PencilSimpleIcon data-icon="inline-start" />
                              Edit
                            </Button>
                            <Button
                              size="sm"
                              variant="outline"
                              disabled={!canWrite}
                              onClick={() => setRetiring(record)}
                            >
                              <TrashIcon data-icon="inline-start" />
                              Retire
                            </Button>
                          </div>
                        </TableCell>
                      )}
                    </TableRow>
                  )
                })}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      </DataState>

      {kind && (
        <Dialog
          open={editor !== null}
          onOpenChange={(open) => !open && !submitting && closeEditor()}
        >
          <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-2xl">
            <DialogHeader>
              <DialogTitle>
                {editor === "new" ? "Add" : "Edit"} {title}
              </DialogTitle>
              <DialogDescription>
                Complete the supported fields below. No JSON editing is
                required.
              </DialogDescription>
            </DialogHeader>
            {editor && (
              <form
                key={`${kind}-${editor === "new" ? "new" : editor.id}`}
                onSubmit={save}
              >
                <FieldGroup>
                  <ResourceForm
                    kind={kind}
                    record={editor === "new" ? undefined : editor}
                    tenantId={tenantId}
                  />
                  {formError && <FieldError>{formError}</FieldError>}
                  <DialogFooter>
                    <Button
                      type="button"
                      variant="outline"
                      disabled={submitting}
                      onClick={closeEditor}
                    >
                      Cancel
                    </Button>
                    <Button type="submit" disabled={submitting}>
                      {submitting && <Spinner data-icon="inline-start" />}
                      Save
                    </Button>
                  </DialogFooter>
                </FieldGroup>
              </form>
            )}
          </DialogContent>
        </Dialog>
      )}

      <AlertDialog
        open={retiring !== null}
        onOpenChange={(open) => !open && !submitting && setRetiring(null)}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Retire this resource?</AlertDialogTitle>
            <AlertDialogDescription>
              {String(retiring?.id ?? "")} will stop being active. Existing
              audit history is preserved.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={submitting}>Cancel</AlertDialogCancel>
            <AlertDialogAction
              variant="destructive"
              disabled={submitting}
              onClick={retire}
            >
              {submitting && <Spinner data-icon="inline-start" />}
              Retire
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  )
}

function ResourceForm({
  kind,
  record,
  tenantId,
}: {
  kind: ResourceKind
  record?: RecordValue
  tenantId: string
}) {
  if (kind === "projects") return <ProjectForm record={record} />
  if (kind === "providers") return <ProviderForm record={record} />
  if (kind === "routing") return <RoutingForm record={record} />
  if (kind === "pricing") return <PricingForm record={record} />
  if (kind === "policies")
    return <PolicyForm record={record} tenantId={tenantId} />
  return <QuotaForm record={record} tenantId={tenantId} />
}

function ProjectForm({ record }: { record?: RecordValue }) {
  return (
    <>
      <TextField
        name="name"
        label="Project name"
        defaultValue={record?.name}
        required
      />
      <SelectField
        name="status"
        label="Status"
        defaultValue={record?.status ?? "active"}
        options={[
          ["active", "Active"],
          ["inactive", "Inactive"],
        ]}
      />
      <SelectField
        name="environment"
        label="Environment"
        defaultValue={String(record?.environment ?? "production")}
        options={[
          ["production", "Production"],
          ["staging", "Staging"],
          ["development", "Development"],
        ]}
      />
    </>
  )
}

function ProviderForm({ record }: { record?: RecordValue }) {
  const [providerType, setProviderType] = React.useState(
    String(record?.provider_type ?? "openai_compatible")
  )
  return (
    <>
      <TextField
        name="id"
        label="Provider ID"
        defaultValue={record?.id}
        disabled={Boolean(record)}
        required
        description="Stable identifier referenced by routes and pricing."
      />
      <TextField
        name="name"
        label="Display name"
        defaultValue={record?.name ?? record?.id}
        required
      />
      <SelectField
        name="provider_type"
        label="Provider type"
        value={providerType}
        onValueChange={(value) => value && setProviderType(value)}
        options={[
          ["openai_compatible", "OpenAI-compatible"],
          ["anthropic", "Anthropic"],
          ["gemini", "Gemini"],
        ]}
      />
      <TextField
        name="base_url"
        label="Base URL"
        type="url"
        defaultValue={record?.base_url}
        placeholder="https://api.example.com"
        required
      />
      <TextField
        name="credential"
        label="Credential"
        type="password"
        autoComplete="new-password"
        required={!record && providerType !== "openai_compatible"}
        description={
          record
            ? "Write-only. Leave blank to keep the existing credential."
            : "Write-only. Required for Anthropic and Gemini."
        }
      />
    </>
  )
}

function RoutingForm({ record }: { record?: RecordValue }) {
  return (
    <>
      <ProviderField name="provider" defaultValue={record?.provider} />
      <TextField
        name="requested_model"
        label="Requested model"
        defaultValue={record?.requested_model}
        placeholder="gateway-model"
        required
      />
      <TextField
        name="upstream_model"
        label="Upstream model"
        defaultValue={record?.upstream_model}
        required
      />
      <TextField
        name="priority"
        label="Priority"
        type="number"
        min={1}
        step={1}
        defaultValue={record?.priority ?? 1}
        required
      />
      <Field orientation="horizontal">
        <Switch
          id="resource-enabled"
          name="enabled"
          defaultChecked={record?.enabled !== false}
        />
        <FieldLabel htmlFor="resource-enabled">Enabled</FieldLabel>
      </Field>
    </>
  )
}

function PricingForm({ record }: { record?: RecordValue }) {
  return (
    <>
      <ProviderField name="provider_id" defaultValue={record?.provider_id} />
      <TextField
        name="upstream_model"
        label="Upstream model"
        defaultValue={record?.upstream_model}
        required
      />
      <div className="grid gap-4 sm:grid-cols-2">
        <TextField
          name="input_cost_per_million"
          label="Input cost / million"
          type="number"
          min={0}
          step="any"
          defaultValue={record?.input_cost_per_million}
          required
        />
        <TextField
          name="output_cost_per_million"
          label="Output cost / million"
          type="number"
          min={0}
          step="any"
          defaultValue={record?.output_cost_per_million}
          required
        />
        <TextField
          name="cached_input_cost_per_million"
          label="Cached input cost / million"
          type="number"
          min={0}
          step="any"
          defaultValue={record?.cached_input_cost_per_million}
        />
        <TextField
          name="embedding_cost_per_million"
          label="Embedding cost / million"
          type="number"
          min={0}
          step="any"
          defaultValue={record?.embedding_cost_per_million}
        />
        <TextField
          name="effective_from"
          label="Effective from"
          type="datetime-local"
          defaultValue={
            localDateTime(record?.effective_from) ?? localDateTime(new Date())
          }
          required
        />
        <TextField
          name="effective_until"
          label="Effective until"
          type="datetime-local"
          defaultValue={localDateTime(record?.effective_until)}
        />
      </div>
    </>
  )
}

function PolicyForm({
  record,
  tenantId,
}: {
  record?: RecordValue
  tenantId: string
}) {
  const policy =
    record?.policy && typeof record.policy === "object"
      ? (record.policy as Record<string, unknown>)
      : record
  return (
    <>
      <ScopeFields record={record} tenantId={tenantId} />
      <ListField
        name="allowed_models"
        label="Allowed models"
        defaultValue={policy?.allowed_models}
      />
      <ListField
        name="denied_models"
        label="Denied models"
        defaultValue={policy?.denied_models}
      />
      <ListField
        name="allowed_operations"
        label="Allowed operations"
        defaultValue={policy?.allowed_operations}
        placeholder="chat, responses, embedding"
      />
      <div className="grid gap-4 sm:grid-cols-2">
        <LimitField
          name="max_output_tokens"
          label="Max output tokens"
          value={policy?.max_output_tokens}
        />
        <LimitField
          name="concurrent_requests"
          label="Concurrent requests"
          value={policy?.concurrent_requests}
        />
        <LimitField
          name="daily_token_limit"
          label="Daily token limit"
          value={policy?.daily_token_limit}
        />
        <LimitField
          name="monthly_token_limit"
          label="Monthly token limit"
          value={policy?.monthly_token_limit}
        />
      </div>
    </>
  )
}

function QuotaForm({
  record,
  tenantId,
}: {
  record?: RecordValue
  tenantId: string
}) {
  return (
    <>
      <ScopeFields record={record} tenantId={tenantId} />
      <SelectField
        name="period"
        label="Period"
        defaultValue={String(record?.period ?? "day")}
        options={[
          ["minute", "Minute"],
          ["day", "Day"],
          ["month", "Month"],
        ]}
      />
      <FieldDescription>Enter at least one limit.</FieldDescription>
      <div className="grid gap-4 sm:grid-cols-2">
        <LimitField
          name="token_limit"
          label="Token limit"
          value={record?.token_limit}
        />
        <TextField
          name="cost_limit"
          label="Cost limit"
          type="number"
          min={0.000001}
          step="any"
          defaultValue={record?.cost_limit}
        />
        <LimitField
          name="concurrent_limit"
          label="Concurrent limit"
          value={record?.concurrent_limit}
        />
        <LimitField
          name="requests_per_minute"
          label="Requests per minute"
          value={record?.requests_per_minute}
        />
      </div>
    </>
  )
}

function ScopeFields({
  record,
  tenantId,
}: {
  record?: RecordValue
  tenantId: string
}) {
  const { projectId } = useGateway()
  const scopeId = projectId ?? tenantId
  return (
    <div className="grid gap-4 sm:grid-cols-2">
      <SelectField
        name="scope_kind"
        label="Scope"
        defaultValue={String(
          record?.scope_kind ?? (projectId ? "project" : "tenant")
        )}
        options={[
          ["global", "Global"],
          ["tenant", "Tenant"],
          ["project", "Project"],
          ["principal", "Principal"],
          ["virtual_key", "Virtual key"],
        ]}
      />
      <TextField
        name="scope_id"
        label="Scope ID"
        defaultValue={record?.scope_id ?? scopeId}
        required
      />
    </div>
  )
}

function ProviderField({
  name,
  defaultValue,
}: {
  name: string
  defaultValue?: unknown
}) {
  const { tenantId, projectId } = useGateway()
  const providers = useGatewayData<Page<RecordValue>>(
    projectId ? `/admin/providers?tenant_id=${tenantId}` : "/admin/providers"
  )
  const options =
    providers.data?.data
      .filter((provider) =>
        projectId ? provider.tenant_id === tenantId : provider.tenant_id == null
      )
      .map(
        (provider) =>
          [String(provider.id), String(provider.name ?? provider.id)] as [
            string,
            string,
          ]
      ) ?? []
  const value = String(defaultValue ?? options[0]?.[0] ?? "")
  return (
    <SelectField
      key={value}
      name={name}
      label="Provider"
      defaultValue={value}
      options={options}
      description={
        providers.loading
          ? "Loading global providers…"
          : options.length
            ? undefined
            : "Create a global provider first."
      }
    />
  )
}

function TextField({
  name,
  label,
  description,
  defaultValue,
  ...props
}: Omit<React.ComponentProps<typeof Input>, "defaultValue" | "name"> & {
  name: string
  label: string
  description?: string
  defaultValue?: unknown
}) {
  return (
    <Field>
      <FieldLabel htmlFor={`resource-${name}`}>{label}</FieldLabel>
      <Input
        id={`resource-${name}`}
        name={name}
        defaultValue={defaultValue == null ? "" : String(defaultValue)}
        {...props}
      />
      {description && <FieldDescription>{description}</FieldDescription>}
    </Field>
  )
}

function LimitField({
  name,
  label,
  value,
}: {
  name: string
  label: string
  value?: unknown
}) {
  return (
    <TextField
      name={name}
      label={label}
      type="number"
      min={1}
      step={1}
      defaultValue={value}
    />
  )
}

function ListField({
  name,
  label,
  defaultValue,
  placeholder,
}: {
  name: string
  label: string
  defaultValue?: unknown
  placeholder?: string
}) {
  return (
    <Field>
      <FieldLabel htmlFor={`resource-${name}`}>{label}</FieldLabel>
      <Textarea
        id={`resource-${name}`}
        name={name}
        defaultValue={
          Array.isArray(defaultValue) ? defaultValue.join(", ") : ""
        }
        placeholder={placeholder}
        rows={2}
      />
      <FieldDescription>Comma-separated; leave blank for any.</FieldDescription>
    </Field>
  )
}

function SelectField({
  name,
  label,
  options,
  defaultValue,
  value,
  onValueChange,
  description,
}: {
  name: string
  label: string
  options: [string, string][]
  defaultValue?: string
  value?: string
  onValueChange?: (value: string | null) => void
  description?: string
}) {
  return (
    <Field>
      <FieldLabel htmlFor={`resource-${name}`}>{label}</FieldLabel>
      <Select
        name={name}
        items={options.map(([itemValue, itemLabel]) => ({
          value: itemValue,
          label: itemLabel,
        }))}
        defaultValue={value === undefined ? defaultValue : undefined}
        value={value}
        onValueChange={onValueChange}
      >
        <SelectTrigger id={`resource-${name}`} className="w-full">
          <SelectValue placeholder={`Select ${label.toLowerCase()}`} />
        </SelectTrigger>
        <SelectContent>
          <SelectGroup>
            {options.map(([itemValue, itemLabel]) => (
              <SelectItem key={itemValue} value={itemValue}>
                {itemLabel}
              </SelectItem>
            ))}
          </SelectGroup>
        </SelectContent>
      </Select>
      {description && <FieldDescription>{description}</FieldDescription>}
    </Field>
  )
}

function localDateTime(value: unknown) {
  if (!value) return undefined
  const date = value instanceof Date ? value : new Date(String(value))
  if (Number.isNaN(date.valueOf())) return undefined
  const offset = date.getTimezoneOffset() * 60_000
  return new Date(date.valueOf() - offset).toISOString().slice(0, 16)
}

function SystemPage() {
  const state = useGatewayData<RecordValue>("/admin/system")
  return (
    <>
      <PageHeader title="System" />
      <DataState
        loading={state.loading}
        error={state.error}
        onRetry={state.reload}
      >
        <Card>
          <CardHeader>
            <CardTitle>Operational snapshot</CardTitle>
            <CardDescription>Sanitized server-side state.</CardDescription>
          </CardHeader>
          <CardContent>
            <pre className="overflow-auto text-xs whitespace-pre-wrap">
              {JSON.stringify(state.data, null, 2)}
            </pre>
          </CardContent>
        </Card>
      </DataState>
    </>
  )
}

function IntegrationsPage() {
  const { tenantId } = useGateway()
  const webhooks = useGatewayData<Page<RecordValue>>("/admin/billing/webhooks")
  const outbox = useGatewayData<Page<RecordValue>>("/admin/billing/outbox")
  return (
    <>
      <PageHeader title="Billing integrations" />
      <RecordCards title="Webhooks" state={webhooks} />
      <div className="mt-4">
        <RecordCards
          title="Outbox"
          state={outbox}
          action={async (record) => {
            await gatewayFetch(
              `/admin/billing/outbox/${record.event_id}/retry`,
              tenantId,
              { method: "POST" }
            )
            outbox.reload()
          }}
        />
      </div>
    </>
  )
}

function RecordCards({
  title,
  state,
  action,
}: {
  title: string
  state: ReturnType<typeof useGatewayData<Page<RecordValue>>>
  action?: (record: RecordValue) => Promise<void>
}) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>{title}</CardTitle>
      </CardHeader>
      <CardContent>
        <DataState
          loading={state.loading}
          error={state.error}
          empty={state.data?.data.length === 0}
          onRetry={state.reload}
        >
          <div className="flex flex-col gap-3">
            {state.data?.data.map((record, index) => (
              <div
                key={String(record.id ?? record.event_id ?? index)}
                className="flex items-start justify-between gap-3 rounded-md border p-3"
              >
                <pre className="overflow-auto text-xs whitespace-pre-wrap">
                  {JSON.stringify(record, null, 2)}
                </pre>
                {action && !record.delivered_at && (
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => action(record)}
                  >
                    <ArrowClockwiseIcon data-icon="inline-start" />
                    Retry
                  </Button>
                )}
              </div>
            ))}
          </div>
        </DataState>
      </CardContent>
    </Card>
  )
}
