"use client"

import * as React from "react"
import { usePathname, useRouter, useSearchParams } from "next/navigation"
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
import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import {
  Select,
  SelectContent,
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
import { gatewayFetch, type Page } from "@/lib/gateway-api"

type Provider = {
  id: string
  name?: string
  provider_type?: string
  base_url?: string
  credential_configured?: boolean
  available_models?: unknown[]
  enabled?: boolean
  version?: number
  updated_at?: string
}

type Row = Record<string, unknown>

function value(value: unknown, fallback = "—") {
  return value === null || value === undefined || value === ""
    ? fallback
    : String(value)
}

function EmptyProvidersRow() {
  return (
    <TableRow>
      <TableCell colSpan={7} className="h-36 text-center">
        <p className="font-medium">No organization providers</p>
        <p className="mt-1 text-sm text-muted-foreground">
          Configure an organization provider before creating project routes.
        </p>
      </TableCell>
    </TableRow>
  )
}

function HealthDot({ status }: { status: string }) {
  const color =
    status === "healthy"
      ? "bg-emerald-500"
      : status === "degraded"
        ? "bg-amber-500"
        : status === "disabled"
          ? "bg-muted-foreground"
          : "bg-red-500"
  return (
    <span
      className={`inline-block size-2 rounded-full ${color}`}
      title={`Health: ${status}`}
    >
      <span className="sr-only">Health: {status}</span>
    </span>
  )
}

export function ProvidersPage() {
  const session = useGateway()
  const pathname = usePathname()
  const router = useRouter()
  const searchParams = useSearchParams()
  const providers = useGatewayData<Page<Provider>>(
    `/admin/providers?tenant_id=${encodeURIComponent(session.tenantId)}`
  )
  const routes = useGatewayData<Page<Row>>(
    `/admin/model-routes?tenant_id=${encodeURIComponent(session.tenantId)}&project_id=${encodeURIComponent(session.projectId ?? "")}`
  )
  const health = useGatewayData<Row>(
    `/admin/system?tenant_id=${encodeURIComponent(session.tenantId)}`
  )
  const prices = useGatewayData<Page<Row>>(
    `/admin/model-prices?tenant_id=${encodeURIComponent(session.tenantId)}`
  )
  const [editor, setEditor] = React.useState<Provider>()
  const [credentialEditor, setCredentialEditor] = React.useState<Provider>()
  const [modelProvider, setModelProvider] = React.useState<Provider>()
  const [models, setModels] = React.useState<string[]>([])
  const [modelsLoading, setModelsLoading] = React.useState(false)
  const [modelsError, setModelsError] = React.useState("")
  const [pricingModel, setPricingModel] = React.useState<{
    provider: Provider
    model: string
    price?: Row
  }>()
  const [disabling, setDisabling] = React.useState<Provider>()
  const [pending, setPending] = React.useState(false)
  const canWrite =
    session.gatewayAdmin ||
    session.tenantRole === "owner" ||
    session.tenantRole === "admin"
  const linkedProvider = providers.data?.data.find(
    (provider) => provider.id === searchParams.get("provider")
  )
  const activeEditor = editor ?? linkedProvider
  const healthRows = Array.isArray(health.data?.providers)
    ? (health.data.providers as Row[])
    : []

  function setProviderQuery(provider?: Provider) {
    const query = new URLSearchParams(searchParams.toString())
    if (provider) query.set("provider", provider.id)
    else query.delete("provider")
    router.replace(query.size ? `${pathname}?${query}` : pathname)
  }

  function providerPayload(provider: Provider) {
    return {
      tenant_id: session.tenantId,
      name: provider.name ?? provider.id,
      provider_type: provider.provider_type,
      base_url: provider.base_url,
      enabled: provider.enabled !== false,
    }
  }

  async function update(
    provider: Provider,
    body: Record<string, unknown>,
    success: string
  ) {
    setPending(true)
    try {
      await gatewayFetch(
        `/admin/providers/${encodeURIComponent(provider.id)}`,
        session.tenantId,
        {
          method: "PATCH",
          headers: {
            "content-type": "application/json",
            "if-match": `"${provider.version ?? 1}"`,
          },
          body: JSON.stringify(body),
        }
      )
      providers.reload()
      toast.success(success)
      return true
    } catch (error) {
      toast.error(
        error instanceof Error ? error.message : "Provider update failed"
      )
      return false
    } finally {
      setPending(false)
    }
  }

  async function configure(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (!activeEditor) return
    const form = new FormData(event.currentTarget)
    const providerType = String(form.get("provider_type"))
    const saved = await update(
      activeEditor,
      {
        ...providerPayload(activeEditor),
        name: form.get("name"),
        provider_type: providerType,
        base_url:
          providerType === "openai"
            ? "https://api.openai.com/v1/"
            : form.get("base_url"),
      },
      "Provider configured"
    )
    if (saved) {
      setEditor(undefined)
      setProviderQuery()
    }
  }

  async function editCredential(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (!credentialEditor) return
    const form = new FormData(event.currentTarget)
    const saved = await update(
      credentialEditor,
      {
        ...providerPayload(credentialEditor),
        credential: form.get("credential"),
      },
      "Provider credential updated"
    )
    if (saved) setCredentialEditor(undefined)
  }

  async function viewModels(provider: Provider) {
    setModelProvider(provider)
    setModels([])
    setModelsError("")
    setModelsLoading(true)
    try {
      const result = await gatewayFetch<{ data: { id: string }[] }>(
        `/admin/providers/${encodeURIComponent(provider.id)}/models`,
        session.tenantId
      )
      setModels(result.data.map((model) => model.id))
      providers.reload()
    } catch (error) {
      setModelsError(
        error instanceof Error ? error.message : "Provider models unavailable"
      )
    } finally {
      setModelsLoading(false)
    }
  }

  async function disable() {
    if (!disabling) return
    const saved = await update(
      disabling,
      { ...providerPayload(disabling), enabled: false },
      "Provider disabled"
    )
    if (saved) setDisabling(undefined)
  }

  async function savePricing(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (!pricingModel) return
    const form = new FormData(event.currentTarget)
    setPending(true)
    try {
      await gatewayFetch("/admin/model-prices", session.tenantId, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          tenant_id: session.tenantId,
          provider_id: pricingModel.provider.id,
          upstream_model: pricingModel.model,
          input_cost_per_million: Number(form.get("input_cost_per_million")),
          output_cost_per_million: Number(form.get("output_cost_per_million")),
          cached_input_cost_per_million:
            form.get("cached_input_cost_per_million") === ""
              ? undefined
              : Number(form.get("cached_input_cost_per_million")),
          effective_from: new Date().toISOString(),
        }),
      })
      prices.reload()
      setPricingModel(undefined)
      toast.success("Model pricing saved")
    } catch (error) {
      toast.error(
        error instanceof Error ? error.message : "Pricing save failed"
      )
    } finally {
      setPending(false)
    }
  }

  return (
    <>
      <PageHeader title="Providers" />
      <Card>
        <CardContent>
          <DataState
            loading={providers.loading}
            error={providers.error}
            onRetry={providers.reload}
          >
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Provider</TableHead>
                  <TableHead>Provider type</TableHead>
                  <TableHead>Credential status</TableHead>
                  <TableHead>Available models</TableHead>
                  <TableHead>Used by project</TableHead>
                  <TableHead>Last updated</TableHead>
                  <TableHead>Actions</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {!providers.data?.data.length && <EmptyProvidersRow />}
                {providers.data?.data.map((provider) => {
                  const providerHealth = healthRows.find(
                    (row) => value(row.provider_id, "") === provider.id
                  )
                  const used = (routes.data?.data ?? []).filter(
                    (route) =>
                      value(route.provider ?? route.provider_id, "") ===
                      provider.id
                  )
                  const available = Array.isArray(provider.available_models)
                    ? provider.available_models.map(String)
                    : used.map((route) => value(route.upstream_model))
                  const status =
                    provider.enabled === false
                      ? "disabled"
                      : value(providerHealth?.status, "unknown").toLowerCase()
                  return (
                    <TableRow key={provider.id}>
                      <TableCell>
                        <div className="flex items-center gap-2 font-medium">
                          <HealthDot status={status} />
                          {value(provider.name, provider.id)}
                        </div>
                      </TableCell>
                      <TableCell>{value(provider.provider_type)}</TableCell>
                      <TableCell>
                        <StatusBadge
                          status={
                            provider.credential_configured
                              ? "Configured"
                              : "Missing"
                          }
                        />
                      </TableCell>
                      <TableCell className="max-w-56 truncate">
                        {available.length
                          ? [...new Set(available)].join(", ")
                          : "Not reported"}
                      </TableCell>
                      <TableCell className="max-w-56 truncate">
                        {used.length
                          ? used
                              .map((route) => value(route.requested_model))
                              .join(", ")
                          : "Not routed"}
                      </TableCell>
                      <TableCell>
                        {provider.updated_at
                          ? new Date(provider.updated_at).toLocaleString()
                          : "Never"}
                      </TableCell>
                      <TableCell>
                        <div className="flex flex-wrap gap-2">
                          <Button
                            size="sm"
                            variant="outline"
                            disabled={!canWrite}
                            onClick={() => {
                              setEditor(provider)
                              setProviderQuery(provider)
                            }}
                          >
                            Configure
                          </Button>
                          <Button
                            size="sm"
                            variant="outline"
                            disabled={!canWrite}
                            onClick={() => setCredentialEditor(provider)}
                          >
                            Edit credentials
                          </Button>
                          <Button
                            size="sm"
                            variant="outline"
                            onClick={() => viewModels(provider)}
                          >
                            View models
                          </Button>
                          <Button
                            size="sm"
                            variant="outline"
                            disabled={!canWrite || provider.enabled === false}
                            onClick={() => setDisabling(provider)}
                          >
                            Disable
                          </Button>
                        </div>
                      </TableCell>
                    </TableRow>
                  )
                })}
              </TableBody>
            </Table>
          </DataState>
        </CardContent>
      </Card>

      <Dialog
        open={Boolean(activeEditor)}
        onOpenChange={(open) => {
          if (!open && !pending) {
            setEditor(undefined)
            setProviderQuery()
          }
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Configure provider</DialogTitle>
            <DialogDescription>
              Update provider metadata and connection details.
            </DialogDescription>
          </DialogHeader>
          {activeEditor && (
            <form key={activeEditor.id} onSubmit={configure}>
              <FieldGroup>
                <Field>
                  <FieldLabel htmlFor="provider-name">Display name</FieldLabel>
                  <Input
                    id="provider-name"
                    name="name"
                    defaultValue={activeEditor.name ?? activeEditor.id}
                    required
                  />
                </Field>
                <Field>
                  <FieldLabel>Provider type</FieldLabel>
                  <Select
                    name="provider_type"
                    defaultValue={
                      activeEditor.provider_type ?? "openai_compatible"
                    }
                  >
                    <SelectTrigger className="w-full">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="openai">OpenAI</SelectItem>
                      <SelectItem value="openai_compatible">
                        OpenAI-compatible
                      </SelectItem>
                      <SelectItem value="anthropic">Anthropic</SelectItem>
                      <SelectItem value="gemini">Gemini</SelectItem>
                    </SelectContent>
                  </Select>
                </Field>
                <Field>
                  <FieldLabel htmlFor="provider-url">Base URL</FieldLabel>
                  <Input
                    id="provider-url"
                    name="base_url"
                    type="url"
                    defaultValue={activeEditor.base_url}
                    required
                  />
                </Field>
                <DialogFooter>
                  <Button type="submit" disabled={pending}>
                    {pending ? "Saving…" : "Save"}
                  </Button>
                </DialogFooter>
              </FieldGroup>
            </form>
          )}
        </DialogContent>
      </Dialog>

      <Dialog
        open={Boolean(credentialEditor)}
        onOpenChange={(open) =>
          !open && !pending && setCredentialEditor(undefined)
        }
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Edit credentials</DialogTitle>
            <DialogDescription>
              The replacement credential is write-only and is never returned.
            </DialogDescription>
          </DialogHeader>
          {credentialEditor && (
            <form key={credentialEditor.id} onSubmit={editCredential}>
              <FieldGroup>
                <Field>
                  <FieldLabel htmlFor="provider-credential">
                    New credential
                  </FieldLabel>
                  <Input
                    id="provider-credential"
                    name="credential"
                    type="password"
                    autoComplete="new-password"
                    required
                  />
                </Field>
                <DialogFooter>
                  <Button type="submit" disabled={pending}>
                    {pending ? "Saving…" : "Save credential"}
                  </Button>
                </DialogFooter>
              </FieldGroup>
            </form>
          )}
        </DialogContent>
      </Dialog>

      <Dialog
        open={Boolean(modelProvider)}
        onOpenChange={(open) => !open && setModelProvider(undefined)}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>
              Models for {value(modelProvider?.name, modelProvider?.id)}
            </DialogTitle>
            <DialogDescription>
              Models currently reported by the provider runtime.
            </DialogDescription>
          </DialogHeader>
          {modelsLoading ? (
            <p className="text-sm text-muted-foreground">Loading models…</p>
          ) : modelsError ? (
            <p className="text-sm text-destructive">{modelsError}</p>
          ) : models.length ? (
            <ul className="max-h-80 space-y-2 overflow-y-auto text-sm">
              {models.map((model) => {
                const price = prices.data?.data.find(
                  (entry) =>
                    value(entry.provider_id, "") === modelProvider?.id &&
                    value(entry.upstream_model, "") === model
                )
                return (
                  <li
                    key={model}
                    className="flex items-center justify-between gap-3 rounded-md border px-3 py-2"
                  >
                    <div>
                      <p className="font-mono">{model}</p>
                      <p className="text-xs text-muted-foreground">
                        {price
                          ? `$${value(price.input_cost_per_million)} input / $${value(price.output_cost_per_million)} output per 1M`
                          : "Unpriced"}
                      </p>
                    </div>
                    {canWrite && modelProvider && (
                      <Button
                        size="sm"
                        variant="outline"
                        onClick={() =>
                          setPricingModel({
                            provider: modelProvider,
                            model,
                            price,
                          })
                        }
                      >
                        {price ? "Update price" : "Set price"}
                      </Button>
                    )}
                  </li>
                )
              })}
            </ul>
          ) : (
            <p className="text-sm text-muted-foreground">
              No models were reported.
            </p>
          )}
        </DialogContent>
      </Dialog>

      <Dialog
        open={Boolean(pricingModel)}
        onOpenChange={(open) => !open && !pending && setPricingModel(undefined)}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Model pricing</DialogTitle>
            <DialogDescription>
              USD cost per one million tokens for {pricingModel?.model}.
            </DialogDescription>
          </DialogHeader>
          <form onSubmit={savePricing}>
            <FieldGroup>
              <Field>
                <FieldLabel>Input cost / 1M tokens</FieldLabel>
                <Input
                  name="input_cost_per_million"
                  type="number"
                  min={0}
                  step="any"
                  defaultValue={value(
                    pricingModel?.price?.input_cost_per_million,
                    ""
                  )}
                  required
                />
              </Field>
              <Field>
                <FieldLabel>Output cost / 1M tokens</FieldLabel>
                <Input
                  name="output_cost_per_million"
                  type="number"
                  min={0}
                  step="any"
                  defaultValue={value(
                    pricingModel?.price?.output_cost_per_million,
                    ""
                  )}
                  required
                />
              </Field>
              <Field>
                <FieldLabel>Cached input cost / 1M tokens</FieldLabel>
                <Input
                  name="cached_input_cost_per_million"
                  type="number"
                  min={0}
                  step="any"
                  defaultValue={value(
                    pricingModel?.price?.cached_input_cost_per_million,
                    ""
                  )}
                />
              </Field>
              <DialogFooter>
                <Button type="submit" disabled={pending}>
                  {pending ? "Saving…" : "Save pricing"}
                </Button>
              </DialogFooter>
            </FieldGroup>
          </form>
        </DialogContent>
      </Dialog>

      <AlertDialog
        open={Boolean(disabling)}
        onOpenChange={(open) => !open && !pending && setDisabling(undefined)}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Disable this provider?</AlertDialogTitle>
            <AlertDialogDescription>
              {value(disabling?.name, disabling?.id)} will no longer be
              available for inference until it is re-enabled.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={pending}>Cancel</AlertDialogCancel>
            <AlertDialogAction
              variant="destructive"
              disabled={pending}
              onClick={disable}
            >
              {pending ? "Disabling…" : "Disable"}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  )
}
