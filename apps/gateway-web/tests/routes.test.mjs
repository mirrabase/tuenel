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
  "/operator/providers",
  "/operator/routing",
  "/operator/pricing",
  "/operator/policies",
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
    "components/pages/workspace-pages.tsx",
    "components/pages/mcp-pages.tsx",
    "components/pages/security-pages.tsx",
    "lib/mock-store.ts",
  ]
  const source = files
    .map((file) => readFileSync(join(root, file), "utf8"))
    .join("\n")
  assert.equal(/localStorage|sessionStorage/.test(source), false)
  assert.match(source, /fetch\s*\(|gatewayFetch/)
})

test("fixtures contain no complete production-shaped virtual key", () => {
  const source = readFileSync(join(root, "lib/mock-data.ts"), "utf8")
  assert.equal(/vk_live_[a-zA-Z0-9]{20,}/.test(source), false)
  assert.match(source, /••••/)
})
