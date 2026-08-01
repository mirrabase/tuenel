import assert from "node:assert/strict"
import { existsSync, readFileSync } from "node:fs"
import { test } from "node:test"
import { join } from "node:path"

const root = new URL("..", import.meta.url).pathname.replace(/^\/(.:)/, "$1")
const routes = [
  "/",
  "/projects",
  "/projects/new",
  "/usage",
  "/team",
  "/providers",
  "/billing",
  "/settings",
  "/operator",
  "/operator/tenants",
  "/operator/projects",
  "/operator/providers",
  "/operator/routing",
  "/operator/pricing",
  "/operator/policies",
  "/operator/quotas",
  "/operator/ledger",
  "/operator/system",
  "/operator/integrations",
  "/operator/mcp",
  "/operator/mcp/policies",
  "/operator/approvals",
  "/operator/security",
  "/operator/security/policies",
]

test("every console route is mapped by the scoped catch-all", () => {
  const scopedPage = readFileSync(
    join(root, "app/[locale]/[tenantId]/[[...slug]]/page.tsx"),
    "utf8"
  )
  for (const route of routes) {
    const key = route === "/" ? "" : route.slice(1)
    assert.match(scopedPage, new RegExp(`(?:["'])?${key}(?:["'])?:`))
  }
  for (const route of routes.filter((route) => route !== "/")) {
    assert.equal(existsSync(join(root, `app${route}/page.tsx`)), false)
  }
  assert.equal(existsSync(join(root, "app/[locale]/page.tsx")), true)
  assert.match(scopedPage, /project-overview/)
  assert.match(scopedPage, /projects\/new/)
  for (const [route, page] of [
    ["team", "organization-team"],
    ["providers", "organization-providers"],
    ["usage", "organization-usage"],
    ["billing", "organization-billing"],
    ["settings", "organization-settings"],
  ])
    assert.match(scopedPage, new RegExp(`${route}: [\"']${page}[\"']`))
  assert.match(scopedPage, /usage: "project-usage"/)
  assert.match(scopedPage, /providers: "project-providers"/)
  assert.match(scopedPage, /settings: "project-settings"/)
  for (const [route, page] of [
    ["routing", "project-routing"],
    ["policies", "project-policies"],
    ["integrations", "project-integrations"],
  ])
    assert.match(scopedPage, new RegExp(`${route}: [\"']${page}[\"']`))
  assert.doesNotMatch(scopedPage, /redirect\([^)]*projects/)
  assert.match(scopedPage, /if \(!kind\) notFound\(\)/)
})

test("project console uses gateway terminology and truthful data states", () => {
  const source = readFileSync(
    join(root, "components/pages/project-console-pages.tsx"),
    "utf8"
  )
  for (const title of [
    "API Keys",
    "Models",
    "Routing",
    "Usage & Cost",
    "Requests",
    "Audit Logs",
    "Policies",
    "Integrations",
    "Project Settings",
  ])
    assert.match(source, new RegExp(`title=[\"']${title}[\"']`))
  assert.doesNotMatch(
    source,
    /gpt-4o|claude-3|gemini-1\.5|t4g\.nano|CPU 0%|POSTGRES/
  )
  assert.match(source, /Prompts, responses, credentials/)
  assert.match(source, /AlertDialog/)
  assert.match(
    source,
    /\/admin\/providers\/\$\{encodeURIComponent\(selectedProvider\)\}\/models/
  )
  assert.match(source, /<datalist id="provider-models">/)
  assert.match(source, /Search or enter a custom model ID/)
  assert.match(source, /function useRangePath\(/)
  assert.match(source, /const \[now\] = React\.useState\(Date\.now\)/)
  assert.doesNotMatch(source, /function rangePath\(/)
  assert.match(source, /value=\{defaultAlias\}/)
  assert.match(source, /setSelectedDefaultAlias\(value\)/)
})

test("provider inventory and health are separate interfaces", () => {
  const providers = readFileSync(
    join(root, "components/pages/providers-page.tsx"),
    "utf8"
  )
  const health = readFileSync(
    join(root, "components/pages/provider-health-page.tsx"),
    "utf8"
  )
  const gatewayPage = readFileSync(
    join(root, "components/gateway-page.tsx"),
    "utf8"
  )

  for (const column of [
    "Provider type",
    "Credential status",
    "Available models",
    "Used by project",
    "Last updated",
    "Actions",
  ])
    assert.match(providers, new RegExp(column))
  for (const action of [
    "Configure",
    "Edit credentials",
    "View models",
    "Disable",
  ])
    assert.match(providers, new RegExp(action))
  assert.doesNotMatch(providers, /Check health/)

  for (const metric of [
    "Healthy providers",
    "Degraded providers",
    "Average success rate",
    "Average p95 latency",
  ])
    assert.match(health, new RegExp(metric))
  for (const column of [
    "Current status",
    "Success rate",
    "Error rate",
    "p50 latency",
    "p95 latency",
    "Last successful request",
    "Last failure",
  ])
    assert.match(health, new RegExp(column))
  for (const chart of ["Availability", "Latency", "Requests and errors"])
    assert.match(health, new RegExp(chart))
  assert.match(health, /Run health check/)
  assert.match(health, /provider-health/)
  assert.match(health, /providers\?provider=/)
  assert.doesNotMatch(
    health,
    /Credential|Available models|Used by project|Configure organization providers/
  )
  assert.doesNotMatch(health, /<ProvidersPage/)
  assert.match(gatewayPage, /pages\/providers-page/)
  assert.match(gatewayPage, /pages\/provider-health-page/)
})

test("project overview is a compact real-data routing dashboard", () => {
  const source = readFileSync(
    join(root, "components/pages/project-console-pages.tsx"),
    "utf8"
  )
  const overview = source.slice(
    source.indexOf("export function ProjectOverviewPage"),
    source.indexOf("type VirtualKey")
  )
  const topology = readFileSync(
    join(root, "components/routing-topology.tsx"),
    "utf8"
  )

  for (const section of [
    "Routing topology",
    "Project summary",
    "Recent requests",
    "Provider health",
    "Usage trend",
  ])
    assert.match(overview, new RegExp(section))
  for (const duplicate of [
    "Project configuration",
    "API Gateway",
    "Deployment mode",
    "Failures",
  ])
    assert.doesNotMatch(overview, new RegExp(duplicate))
  assert.equal(
    (
      overview.match(
        /xl:grid-cols-\[minmax\(0,2\.2fr\)_minmax\(320px,0\.8fr\)\]/g
      ) ?? []
    ).length,
    2
  )
  assert.doesNotMatch(
    overview,
    /xl:grid-cols-\[minmax\(0,1\.[48]fr\)|xl:sticky/
  )

  assert.match(topology, /from "@xyflow\/react"/)
  assert.match(topology, /<ReactFlow/)
  assert.match(topology, /<Controls/)
  assert.match(topology, /<Background/)
  assert.match(topology, /Number\(left\.priority/)
  assert.match(topology, /targetIndex === 0 \? "Primary"/)
  assert.match(topology, /`Fallback \$\{targetIndex\}`/)
  assert.match(topology, /title="Tuenel Gateway"/)
  assert.match(topology, /proOptions=\{\{ hideAttribution: true \}\}/)
  assert.doesNotMatch(topology, /title="Tunel Gateway"/)
  assert.doesNotMatch(topology, /Gemini|Qwen|OpenAI/)
})

test("project navigation is grouped, unique, searchable, and route-complete", () => {
  const shell = readFileSync(join(root, "components/console-shell.tsx"), "utf8")
  const scopedPage = readFileSync(
    join(root, "app/[locale]/[tenantId]/[[...slug]]/page.tsx"),
    "utf8"
  )
  for (const group of [
    "Project",
    "Gateway",
    "Observability",
    "Governance",
    "Developer",
  ])
    assert.match(shell, new RegExp(`label: [\"']${group}[\"']`))

  const expected = [
    ["", "Overview", "HouseLineIcon"],
    ["/playground", "Playground", "TerminalWindowIcon"],
    ["/providers", "Providers", "CloudIcon"],
    ["/models", "Models", "CubeIcon"],
    ["/routing", "Routing", "GitBranchIcon"],
    ["/keys", "API Keys", "KeyIcon"],
    ["/logs", "Requests", "PulseIcon"],
    ["/usage", "Usage & Cost", "ChartLineUpIcon"],
    ["/health", "Provider Health", "HeartbeatIcon"],
    ["/policies", "Policies", "ShieldChevronIcon"],
    ["/audit", "Audit Logs", "ClipboardTextIcon"],
    ["/integrations", "Integrations", "PlugsConnectedIcon"],
    ["/settings", "Project Settings", "SlidersHorizontalIcon"],
  ]
  for (const [path, label, icon] of expected)
    assert.match(
      shell,
      new RegExp(
        `path: [\"']${path.replace("/", "\\/")}[\"'],\\s*label: [\"']${label}[\"'],\\s*icon: ${icon}`
      )
    )
  assert.equal(new Set(expected.map((item) => item[2])).size, expected.length)
  assert.equal((shell.match(/<SidebarTrigger/g) ?? []).length, 1)
  assert.doesNotMatch(shell, /SidebarFooter|Project \{projectId.*slice/)
  assert.match(shell, /event\.metaKey \|\| event\.ctrlKey/)
  assert.match(shell, /<Command className=/)
  assert.match(shell, /<details/)
  assert.doesNotMatch(shell, /CommandDialog|DropdownMenuTrigger/)
  assert.match(scopedPage, /health: "provider-health"/)
  assert.match(scopedPage, /"provider-health": "Provider Health"/)
  assert.match(scopedPage, /playground: "Playground"/)
})

test("playground is a project-scoped vendor-agnostic testing workspace", () => {
  const routeSource = readFileSync(
    join(root, "components/pages/workspace-pages.tsx"),
    "utf8"
  )
  const playground = readFileSync(
    join(root, "components/playground-workspace.tsx"),
    "utf8"
  )
  const shell = readFileSync(join(root, "components/console-shell.tsx"), "utf8")
  assert.match(routeSource, /return <PlaygroundWorkspace \/>/)
  assert.match(playground, /h-full min-h-0 flex-1/)
  assert.doesNotMatch(playground, /min-h-\[680px\]/)
  assert.match(playground, /w-\[350px\]/)
  assert.match(playground, /overflow-y-auto overscroll-contain/)
  assert.match(playground, /sticky bottom-0 z-10 shrink-0/)
  assert.match(playground, /sticky top-0 z-10 flex h-10/)
  assert.match(shell, /h-svh overflow-hidden/)
  assert.match(shell, /min-h-0 overflow-hidden p-4/)
  assert.match(playground, /sm:max-w-\[520px\]/)
  for (const section of [
    "Prompt",
    "Model",
    "Parameters",
    "Variables",
    "Tools",
    "Advanced",
  ])
    assert.match(playground, new RegExp(`title="${section}"`))
  for (const operation of ["chat", "responses", "embeddings"])
    assert.match(playground, new RegExp(`value="${operation}"`))
  for (const inspectorTab of [
    "request",
    "response",
    "headers",
    "timing",
    "routing",
  ])
    assert.match(playground, new RegExp(`"${inspectorTab}"`))
  assert.match(playground, /stream_options: streaming/)
  assert.match(playground, /include_usage: true/)
  assert.match(playground, /if \(abort\.signal\.aborted\)/)
  assert.match(playground, /TUNEL_API_KEY/)
  assert.match(playground, /Coming soon/)
  assert.match(playground, /function MarkdownContent/)
  assert.match(playground, /renderInlineMarkdown/)
  assert.match(playground, /list-disc/)
  assert.match(playground, /<strong/)
  assert.doesNotMatch(playground, /dangerouslySetInnerHTML/)
  assert.match(playground, /sessionStorage\.setItem\(safeDraftKey/)
  assert.doesNotMatch(
    playground.slice(
      playground.indexOf("const safeDraft ="),
      playground.indexOf("sessionStorage.setItem")
    ),
    /promptMessages|conversation|headers|rawOverrides|api.?key/i
  )
  assert.doesNotMatch(
    playground,
    /gpt-|claude-|gemini-|\bplans?\b|\bbilling\b|\bsubscription\b/i
  )
})

test("usage keeps timeline charts and scopes stacked breakdown charts to a scroller", () => {
  const source = readFileSync(
    join(root, "components/pages/project-console-pages.tsx"),
    "utf8"
  )
  const usage = source.slice(
    source.indexOf("export function UsageCostPage"),
    source.indexOf("export function RequestsPage")
  )
  assert.match(source, /function ChartShell/)
  assert.match(source, /function BreakdownTimeSeriesCard/)
  assert.match(source, /stackId="requests"/)
  assert.equal((usage.match(/<ChartShell/g) ?? []).length, 3)
  assert.equal((usage.match(/<BreakdownTimeSeriesCard/g) ?? []).length, 1)
  assert.match(source, /overflow-x-auto overscroll-x-contain/)
  assert.doesNotMatch(usage, /min-w-0 overflow-x-auto pb-2/)
})

test("shared buttons and client-only values are hydration safe", () => {
  const button = readFileSync(join(root, "components/ui/button.tsx"), "utf8")
  const shell = readFileSync(join(root, "components/console-shell.tsx"), "utf8")
  const shared = readFileSync(join(root, "components/pages/shared.tsx"), "utf8")
  const projectConsole = readFileSync(
    join(root, "components/pages/project-console-pages.tsx"),
    "utf8"
  )
  const projectPages = readFileSync(
    join(root, "components/pages/project-pages.tsx"),
    "utf8"
  )
  assert.match(button, /nativeButton=\{nativeButton \?\? !render\}/)
  assert.doesNotMatch(shell, /\{resolvedTheme === "dark" \?/)
  assert.match(shell, /hidden dark:block/)
  assert.match(shared, /useGatewayEndpoint/)
  assert.doesNotMatch(projectConsole, /typeof window/)
  assert.doesNotMatch(projectPages, /typeof window/)
})

test("public gateway URLs come from runtime deployment configuration", () => {
  const provider = readFileSync(
    join(root, "components/gateway-provider.tsx"),
    "utf8"
  )
  const layout = readFileSync(
    join(root, "app/[locale]/[tenantId]/layout.tsx"),
    "utf8"
  )
  const shared = readFileSync(join(root, "components/pages/shared.tsx"), "utf8")
  const projectConsole = readFileSync(
    join(root, "components/pages/project-console-pages.tsx"),
    "utf8"
  )
  const apiReference = readFileSync(
    join(root, "components/api-reference.tsx"),
    "utf8"
  )

  assert.match(provider, /gatewayEndpoint: string/)
  assert.match(layout, /process\.env\.GATEWAY_PUBLIC_URL/)
  assert.match(shared, /return useGateway\(\)\.gatewayEndpoint/)
  assert.ok(shared.includes('replace(/\\/v1$/, "")'))
  assert.doesNotMatch(shared, /window\.location\.origin/)
  assert.match(projectConsole, /const baseUrl = endpoint\.replace/)
  assert.doesNotMatch(projectConsole, /endpoint\.replace[^\n]+\/v1/)
  assert.ok(apiReference.includes("`${gatewayOrigin}${activeEndpoint.path}`"))
  assert.doesNotMatch(apiReference, /\{gatewayEndpoint\}\/v1/)
})

test("browser credentials are never persisted in script-readable storage", () => {
  const files = [
    "components/console-shell.tsx",
    "components/gateway-provider.tsx",
    "components/pages/workspace-pages.tsx",
    "components/pages/platform-pages.tsx",
    "components/pages/providers-page.tsx",
    "components/pages/provider-health-page.tsx",
    "components/pages/mcp-pages.tsx",
    "components/pages/security-pages.tsx",
    "lib/gateway-api.ts",
  ]
  const source = files
    .map((file) => readFileSync(join(root, file), "utf8"))
    .join("\n")
  assert.equal(/localStorage|sessionStorage/.test(source), false)
  assert.equal(
    /mock-store|mock-data|MockProvider|useMockGateway/.test(source),
    false
  )
  assert.match(source, /fetch\s*\(|gatewayFetch/)
})

test("organization pages keep their structure and project scope is explicit", () => {
  const organizationPages = [
    "components/pages/organization-pages.tsx",
    "components/pages/members-page.tsx",
  ]
    .map((file) => readFileSync(join(root, file), "utf8"))
    .join("\n")
  for (const component of [
    "OrganizationTeamPage",
    "OrganizationProvidersPage",
    "OrganizationUsagePage",
    "OrganizationBillingPage",
    "OrganizationSettingsPage",
  ])
    assert.match(organizationPages, new RegExp(`function ${component}`))
  for (const shell of [
    "Project comparison",
    "Pending invitations",
    "Invoice history",
    "Danger zone",
    "Organization providers",
  ])
    assert.match(organizationPages, new RegExp(shell))

  const api = readFileSync(join(root, "lib/gateway-api.ts"), "utf8")
  const keys = readFileSync(
    join(root, "components/pages/workspace-pages.tsx"),
    "utf8"
  )
  assert.doesNotMatch(api, /window\.location\.pathname/)
  assert.match(keys, /virtual-keys\?tenant_id=.*project_id=/)
  assert.match(keys, /project_id: projectId/)
})

test("operator mutations use dedicated forms instead of raw JSON", () => {
  const source = readFileSync(
    join(root, "components/pages/platform-pages.tsx"),
    "utf8"
  )
  assert.doesNotMatch(source, /JSON fields|JSON\.parse\(details\)/)
  for (const form of [
    "ProjectForm",
    "ProviderForm",
    "RoutingForm",
    "PricingForm",
    "PolicyForm",
    "QuotaForm",
  ])
    assert.match(source, new RegExp(`function ${form}`))
})

test("billing is managed-only in navigation and server routing", () => {
  const shell = readFileSync(join(root, "components/console-shell.tsx"), "utf8")
  const page = readFileSync(
    join(root, "app/[locale]/[tenantId]/[[...slug]]/page.tsx"),
    "utf8"
  )
  assert.match(shell, /item\.path !== ["']\/billing["']/)
  assert.match(shell, /session\.edition === ["']managed["']/)
  assert.match(page, /kind === ["']organization-billing["']/)
  assert.match(page, /getTenantCapabilities\(tenantId\)/)
  assert.match(page, /edition !== ["']managed["']/)
  assert.match(page, /notFound\(\)/)
})

test("managed billing renders unlimited values and roadmap features truthfully", () => {
  const billing = readFileSync(
    join(root, "components/pages/organization-pages.tsx"),
    "utf8"
  )
  const shell = readFileSync(join(root, "components/console-shell.tsx"), "utf8")
  assert.match(billing, /if \(value === null\) return ["']Unlimited["']/)
  assert.match(billing, /routedTokenLimit !== null/)
  assert.match(billing, /API key devices \/ credentials/)
  assert.match(billing, /custom_domain/)
  assert.match(billing, /Coming Soon/)
  assert.match(billing, /Confirm plan change/)
  assert.match(billing, /standard\s+proration/)
  assert.match(billing, /setPlanConfirmation\(tier\)/)
  assert.match(shell, /Unlimited plan/)
})

test("providers discover upstream models before routes and expose explicit pricing", () => {
  const providers = readFileSync(
    join(root, "components/pages/providers-page.tsx"),
    "utf8"
  )
  const organization = readFileSync(
    join(root, "components/pages/organization-pages.tsx"),
    "utf8"
  )
  assert.match(providers, /provider\.id\)\}\/models/)
  assert.match(providers, /Unpriced/)
  assert.match(providers, /Set price/)
  assert.match(providers, /input_cost_per_million/)
  assert.match(organization, /available_models/)
  assert.match(organization, /syncProvider\(String\(form\.get\(["']id["']\)\)\)/)
})

test("one canonical logo brands metadata, auth, picker, and sidebar", () => {
  const logo = readFileSync(join(root, "public/logo.svg"), "utf8")
  const metadata = readFileSync(join(root, "app/layout.tsx"), "utf8")
  const brand = readFileSync(join(root, "components/brand.tsx"), "utf8")
  const auth = readFileSync(join(root, "components/auth-form.tsx"), "utf8")
  const authLayout = readFileSync(
    join(root, "app/[locale]/(auth)/layout.tsx"),
    "utf8"
  )
  const picker = readFileSync(
    join(root, "components/organization-picker.tsx"),
    "utf8"
  )
  const shell = readFileSync(join(root, "components/console-shell.tsx"), "utf8")

  assert.match(logo, /fill:#193cb8/)
  assert.match(logo, /fill:#ffffff/)
  assert.match(metadata, /icon: ["']\/logo\.svg["']/)
  assert.match(brand, /src=["']\/logo\.svg["']/)
  assert.match(brand, /alt=["']Tuenel logo["']/)
  for (const source of [auth, authLayout, picker, shell])
    assert.match(source, /<Brand/)
  assert.equal(existsSync(join(root, "public/tuenel-logo.svg")), false)
  assert.equal(existsSync(join(root, "app/favicon.ico")), false)
})
