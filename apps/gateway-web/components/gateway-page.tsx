import {
  ApprovalsPage,
  McpExplorerPage,
  McpPoliciesPage,
  McpRegistryPage,
} from "@/components/pages/mcp-pages"
import {
  PlatformPage,
  type PlatformKind,
} from "@/components/pages/platform-pages"
import {
  SecurityOperationsPage,
  SecurityPoliciesPage,
} from "@/components/pages/security-pages"
import {
  DocsPage,
  KeysPage,
  ModelsPage,
  OverviewPage,
  PlaygroundPage,
  UsagePage,
} from "@/components/pages/workspace-pages"
import { MembersPage } from "@/components/pages/members-page"

export type PageKind =
  | "tenant-overview"
  | "playground"
  | "models"
  | "keys"
  | "usage"
  | "docs"
  | "members"
  | "mcp-explorer"
  | "mcp-registry"
  | "mcp-policies"
  | "approvals"
  | "security"
  | "security-policies"
  | "operator-overview"
  | PlatformKind

export function GatewayPage({ kind }: { kind: PageKind }) {
  if (kind === "tenant-overview") return <OverviewPage />
  if (kind === "operator-overview") return <OverviewPage operator />
  if (kind === "playground") return <PlaygroundPage />
  if (kind === "models") return <ModelsPage />
  if (kind === "keys") return <KeysPage />
  if (kind === "usage") return <UsagePage />
  if (kind === "docs") return <DocsPage />
  if (kind === "members") return <MembersPage />
  if (kind === "mcp-explorer") return <McpExplorerPage />
  if (kind === "mcp-registry") return <McpRegistryPage />
  if (kind === "mcp-policies") return <McpPoliciesPage />
  if (kind === "approvals") return <ApprovalsPage />
  if (kind === "security") return <SecurityOperationsPage />
  if (kind === "security-policies") return <SecurityPoliciesPage />
  return <PlatformPage kind={kind} />
}
