"use client"

import {
  ActivityIcon,
  BuildingsIcon,
  CoinsIcon,
  DatabaseIcon,
  PlugsConnectedIcon,
  ShieldCheckIcon,
  TreeStructureIcon,
} from "@phosphor-icons/react"

import {
  BackendNotice,
  Metric,
  PageHeader,
  StateVariants,
  StatusBadge,
} from "@/components/pages/shared"
import { Button } from "@/components/ui/button"
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import {
  healthChecks,
  integrations,
  modelRoutes,
  policies,
  pricingRules,
  providers,
  reservations,
  tenants,
  usageEvents,
} from "@/lib/mock-data"

export type PlatformKind =
  | "tenants"
  | "providers"
  | "routing"
  | "pricing"
  | "policies"
  | "ledger"
  | "system"
  | "integrations"

const config = {
  tenants: {
    title: "Tenants",
    description: "Mock tenant, project, role, and quota administration.",
    icon: BuildingsIcon,
    columns: ["Tenant", "Quota", "Used", "Status"],
    rows: tenants.map((v) => [v.id, v.quota, v.used, v.status]),
  },
  providers: {
    title: "Providers",
    description:
      "Mock provider adapters, health, and write-only credential state.",
    icon: PlugsConnectedIcon,
    columns: ["Provider", "Endpoint", "Latency", "Status"],
    rows: providers.map((v) => [v.name, v.baseUrl, v.latency, v.status]),
  },
  routing: {
    title: "Routing",
    description: "Mock stable aliases and provider-specific upstream routes.",
    icon: TreeStructureIcon,
    columns: ["Alias", "Provider", "Upstream", "Status"],
    rows: modelRoutes.map((v) => [v.alias, v.provider, v.upstream, v.status]),
  },
  pricing: {
    title: "Pricing",
    description: "Mock versioned token rates and cost projections.",
    icon: CoinsIcon,
    columns: ["Model", "Input", "Output", "Status"],
    rows: pricingRules.map((v) => [v.model, v.input, v.output, v.status]),
  },
  policies: {
    title: "General policies",
    description:
      "Mock model and quota policy administration; backend API is not available.",
    icon: ShieldCheckIcon,
    columns: ["Policy", "Target", "Rule", "Status"],
    rows: policies.map((v) => [v.name, v.target, v.rule, v.status]),
  },
  ledger: {
    title: "Usage ledger",
    description: "Mock usage, cost, and reservation queries.",
    icon: DatabaseIcon,
    columns: ["Record", "Owner/model", "Value", "Status"],
    rows: [
      ...usageEvents.map((v) => [
        v.id,
        v.model,
        `${v.tokens} tokens`,
        v.status,
      ]),
      ...reservations.map((v) => [v.id, v.owner, v.tokens, v.status]),
    ],
  },
  system: {
    title: "System",
    description:
      "Mock health, readiness, metrics, and sanitized runtime state.",
    icon: ActivityIcon,
    columns: ["Check", "Detail", "Value", "Status"],
    rows: healthChecks.map((v) => [v.name, v.detail, v.value, "Available"]),
  },
  integrations: {
    title: "Integrations",
    description:
      "Mock billing outbox, usage delivery, and integrations; delivery never blocks inference.",
    icon: PlugsConnectedIcon,
    columns: ["Integration", "Description", "Delivery", "Status"],
    rows: integrations.map((v) => [
      v.name,
      v.description,
      v.delivery,
      v.status,
    ]),
  },
}

export function PlatformPage({ kind }: { kind: PlatformKind }) {
  const page = config[kind]
  const Icon = page.icon
  return (
    <>
      <PageHeader
        title={page.title}
        description={page.description}
        action={
          <Button disabled>
            <Icon data-icon="inline-start" />
            Simulated create
          </Button>
        }
      />
      <BackendNotice />
      <div className="mt-4 grid gap-3 sm:grid-cols-3">
        <Metric
          label="Backend availability"
          value="Not integrated"
          detail="Full-mock phase"
        />
        <Metric
          label="Fixture snapshot"
          value="v0.3"
          detail="Deterministic browser state"
        />
        <Metric
          label="Persistence"
          value="Session only"
          detail="Reset from shell"
        />
      </div>
      <Card className="mt-4">
        <CardHeader>
          <CardTitle>{page.title} records</CardTitle>
          <CardDescription>
            Mock data; see the root backend-gap document for the minimum future
            contract.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <StateVariants>
            <Table>
              <TableHeader>
                <TableRow>
                  {page.columns.map((column) => (
                    <TableHead key={column}>{column}</TableHead>
                  ))}
                </TableRow>
              </TableHeader>
              <TableBody>
                {page.rows.map((row, index) => (
                  <TableRow key={`${kind}-${index}`}>
                    {row.map((cell, cellIndex) => (
                      <TableCell key={`${cellIndex}-${cell}`}>
                        {cellIndex === row.length - 1 ? (
                          <StatusBadge status={String(cell)} />
                        ) : (
                          String(cell)
                        )}
                      </TableCell>
                    ))}
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </StateVariants>
        </CardContent>
      </Card>
    </>
  )
}
