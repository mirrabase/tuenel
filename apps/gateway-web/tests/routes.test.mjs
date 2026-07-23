import assert from "node:assert/strict"
import { readFileSync } from "node:fs"
import { test } from "node:test"
import { join } from "node:path"

const root = new URL("..", import.meta.url).pathname.replace(/^\/(.:)/, "$1")
const routes = [
  "/",
  "/playground",
  "/models",
  "/keys",
  "/usage",
  "/docs",
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
  "/mcp",
  "/operator/mcp",
  "/operator/mcp/policies",
  "/operator/approvals",
  "/operator/security",
  "/operator/security/policies",
]

test("every console route has a page", () => {
  for (const route of routes) {
    const relative = route === "/" ? "app/page.tsx" : `app${route}/page.tsx`
    assert.doesNotThrow(() => readFileSync(join(root, relative)))
  }
})

test("browser credentials are never persisted in script-readable storage", () => {
  const files = [
    "components/console-shell.tsx",
    "components/gateway-provider.tsx",
    "components/pages/workspace-pages.tsx",
    "components/pages/platform-pages.tsx",
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
