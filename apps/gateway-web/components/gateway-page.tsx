import {
  ApprovalsPage,
  McpExplorerPage,
  McpPoliciesPage,
  McpRegistryPage,
} from "@/components/pages/mcp-pages"
import {
  OrganizationBillingPage,
  OrganizationProvidersPage,
  OrganizationSettingsPage,
  OrganizationTeamPage,
  OrganizationUsagePage,
} from "@/components/pages/organization-pages"
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
  OverviewPage,
  PlaygroundPage,
} from "@/components/pages/workspace-pages"
import {
  ProjectCreationPage,
  ProjectsPage,
} from "@/components/pages/project-pages"
import {
  ApiKeysPage,
  AuditLogsPage,
  IntegrationsPage,
  ModelsPage,
  PoliciesPage,
  ProjectOverviewPage,
  ProjectSettingsPage,
  RequestsPage,
  RoutingPage,
  UsageCostPage,
} from "@/components/pages/project-console-pages"
import { ProviderHealthPage } from "@/components/pages/provider-health-page"
import { ProvidersPage } from "@/components/pages/providers-page"

export type PageKind =
  | "tenant-overview"
  | "projects"
  | "project-create"
  | "project-overview"
  | "playground"
  | "models"
  | "keys"
  | "project-usage"
  | "docs"
  | "organization-team"
  | "organization-providers"
  | "organization-usage"
  | "organization-billing"
  | "organization-settings"
  | "project-providers"
  | "provider-health"
  | "project-settings"
  | "project-routing"
  | "project-policies"
  | "project-integrations"
  | "operator-providers"
  | "logs"
  | "audit-logs"
  | "integrations"
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
  if (kind === "projects") return <ProjectsPage />
  if (kind === "project-create") return <ProjectCreationPage />
  if (kind === "project-overview") return <ProjectOverviewPage />
  if (kind === "operator-overview") return <OverviewPage operator />
  if (kind === "playground") return <PlaygroundPage />
  if (kind === "models") return <ModelsPage />
  if (kind === "keys") return <ApiKeysPage />
  if (kind === "project-usage") return <UsageCostPage />
  if (kind === "docs") return <DocsPage />
  if (kind === "organization-team") return <OrganizationTeamPage />
  if (kind === "organization-providers") return <OrganizationProvidersPage />
  if (kind === "organization-usage") return <OrganizationUsagePage />
  if (kind === "organization-billing") return <OrganizationBillingPage />
  if (kind === "organization-settings") return <OrganizationSettingsPage />
  if (kind === "project-providers") return <ProvidersPage />
  if (kind === "provider-health") return <ProviderHealthPage />
  if (kind === "operator-providers") return <PlatformPage kind="providers" />
  if (kind === "project-settings") return <ProjectSettingsPage />
  if (kind === "logs") return <RequestsPage />
  if (kind === "audit-logs") return <AuditLogsPage />
  if (kind === "project-routing") return <RoutingPage />
  if (kind === "project-policies") return <PoliciesPage />
  if (kind === "project-integrations") return <IntegrationsPage />
  if (kind === "mcp-explorer") return <McpExplorerPage />
  if (kind === "mcp-registry") return <McpRegistryPage />
  if (kind === "mcp-policies") return <McpPoliciesPage />
  if (kind === "approvals") return <ApprovalsPage />
  if (kind === "security") return <SecurityOperationsPage />
  if (kind === "security-policies") return <SecurityPoliciesPage />
  return <PlatformPage kind={kind} />
}
