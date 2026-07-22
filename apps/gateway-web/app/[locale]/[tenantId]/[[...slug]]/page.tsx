import { notFound } from "next/navigation"

import { GatewayPage, type PageKind } from "@/components/gateway-page"

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
  params: Promise<{ slug?: string[] }>
}) {
  const kind = pages[(await params).slug?.join("/") ?? ""]
  if (!kind) notFound()
  return <GatewayPage kind={kind} />
}
