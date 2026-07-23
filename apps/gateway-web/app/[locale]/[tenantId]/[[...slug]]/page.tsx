import { notFound } from "next/navigation"

import { GatewayPage, type PageKind } from "@/components/gateway-page"
import { getSession } from "@/lib/server-auth"

const pages: Record<string, PageKind> = {
  "": "tenant-overview",
  playground: "playground",
  models: "models",
  keys: "keys",
  usage: "usage",
  docs: "docs",
  members: "members",
  mcp: "mcp-explorer",
  operator: "operator-overview",
  "operator/tenants": "tenants",
  "operator/providers": "providers",
  "operator/routing": "routing",
  "operator/pricing": "pricing",
  "operator/policies": "policies",
  "operator/ledger": "ledger",
  "operator/system": "system",
  "operator/integrations": "integrations",
  "operator/mcp": "mcp-registry",
  "operator/mcp/policies": "mcp-policies",
  "operator/approvals": "approvals",
  "operator/security": "security",
  "operator/security/policies": "security-policies",
}

export default async function ScopedPage({
  params,
}: {
  params: Promise<{ tenantId: string; slug?: string[] }>
}) {
  const { tenantId, slug } = await params
  const route = slug?.join("/") ?? ""
  const kind = pages[route]
  if (!kind) notFound()
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
