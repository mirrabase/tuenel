import type { Metadata } from "next"
import { notFound } from "next/navigation"

import { GatewayPage, type PageKind } from "@/components/gateway-page"
import { getSession, projectBelongsToTenant } from "@/lib/server-auth"

const organizationPages: Record<string, PageKind> = {
  "": "projects",
  projects: "projects",
  "projects/new": "project-create",
  team: "organization-team",
  providers: "organization-providers",
  usage: "organization-usage",
  billing: "organization-billing",
  settings: "organization-settings",
}

const projectPages: Record<string, PageKind> = {
  playground: "playground",
  models: "models",
  keys: "keys",
  usage: "project-usage",
  docs: "docs",
  providers: "project-providers",
  health: "provider-health",
  settings: "project-settings",
  logs: "logs",
  audit: "audit-logs",
  integrations: "project-integrations",
  routing: "project-routing",
  policies: "project-policies",
  mcp: "mcp-explorer",
}

const operatorPages: Record<string, PageKind> = {
  operator: "operator-overview",
  "operator/tenants": "tenants",
  "operator/projects": "projects",
  "operator/providers": "operator-providers",
  "operator/routing": "routing",
  "operator/pricing": "pricing",
  "operator/policies": "policies",
  "operator/quotas": "quotas",
  "operator/ledger": "ledger",
  "operator/system": "system",
  "operator/integrations": "integrations",
  "operator/mcp": "mcp-registry",
  "operator/mcp/policies": "mcp-policies",
  "operator/approvals": "approvals",
  "operator/security": "security",
  "operator/security/policies": "security-policies",
}

function resolvePage(parts: string[]) {
  if (parts[0] === "project") {
    const projectId = parts[1]
    if (!projectId?.match(/^[0-9a-f-]{36}$/i)) notFound()
    const route = parts.slice(2).join("/")
    return {
      kind: route ? projectPages[route] : ("project-overview" as PageKind),
      projectId,
      route,
    }
  }
  const route = parts.join("/")
  return {
    kind: organizationPages[route] ?? operatorPages[route],
    projectId: undefined,
    route,
  }
}

const titles: Partial<Record<PageKind, string>> = {
  projects: "Projects",
  "project-create": "New Project",
  "project-overview": "Overview",
  playground: "Playground",
  "organization-team": "Team",
  "organization-providers": "Providers",
  "organization-usage": "Organization Usage",
  "organization-billing": "Billing",
  "organization-settings": "Organization Settings",
  "project-usage": "Usage & Cost",
  "project-providers": "Providers",
  "provider-health": "Provider Health",
  "project-settings": "Project Settings",
  keys: "API Keys",
  models: "Models",
  "project-routing": "Routing",
  "project-policies": "Policies",
  "project-integrations": "Integrations",
  logs: "Requests",
  "audit-logs": "Audit Logs",
}

export async function generateMetadata({
  params,
}: {
  params: Promise<{ slug?: string[] }>
}): Promise<Metadata> {
  const { slug } = await params
  const { kind } = resolvePage(slug ?? [])
  if (!kind) return {}
  return { title: `${titles[kind] ?? "Tuenel"} · Tuenel` }
}

export default async function ScopedPage({
  params,
}: {
  params: Promise<{ locale: string; tenantId: string; slug?: string[] }>
}) {
  const { tenantId, slug } = await params
  const parts = slug ?? []
  const { kind, projectId, route } = resolvePage(parts)
  if (!kind) notFound()
  if (projectId && !(await projectBelongsToTenant(tenantId, projectId)))
    notFound()
  if (route.startsWith("operator")) {
    const session = await getSession()
    const membership = session?.memberships.find(
      (item) => item.tenant_id === tenantId
    )
    if (!session || (!session.gateway_admin && membership?.role === "viewer"))
      notFound()
  }
  return <GatewayPage kind={kind} />
}
