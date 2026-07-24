import assert from "node:assert/strict"
import { existsSync, readFileSync } from "node:fs"
import { join } from "node:path"
import { test } from "node:test"

const webRoot = new URL("..", import.meta.url).pathname.replace(/^\/(.:)/, "$1")
const repoRoot = join(webRoot, "..", "..")
const backend = [
  "crates/gateway-server/src/lib.rs",
  "crates/gateway-server/src/v03.rs",
  "crates/gateway-server/src/admin.rs",
]
  .map((file) => readFileSync(join(repoRoot, file), "utf8"))
  .join("\n")
const backendRoutes = [...backend.matchAll(/\.route\(\s*"([^"]+)"/g)].map(
  (match) => match[1]
)

test("every browser API is represented by the authenticated catch-all console", () => {
  assert.ok(backendRoutes.length > 40)
  assert.equal(
    existsSync(join(webRoot, "app/[locale]/[tenantId]/[[...slug]]/page.tsx")),
    true
  )
  assert.equal(
    existsSync(join(webRoot, "app/api/gateway/[...path]/route.ts")),
    true
  )
  const gaps = readFileSync(
    join(repoRoot, "GATEWAY_WEB_BACKEND_GAPS.md"),
    "utf8"
  )
  assert.match(gaps, /Native `\/mcp`/)
  assert.deepEqual(
    backendRoutes.filter((route) => route === "/mcp"),
    ["/mcp"]
  )
})

test("organization APIs and project-key isolation are enforced by the backend", () => {
  for (const route of [
    "/admin/usage/breakdowns",
    "/admin/provider-health",
    "/admin/billing/overview",
    "/admin/billing/invoices",
    "/admin/providers/{id}/models",
    "/auth/tenants/{tenant_id}",
    "/auth/tenants/{tenant_id}/invitations",
    "/auth/tenants/{tenant_id}/members/{user_id}",
  ])
    assert.ok(backendRoutes.includes(route), `${route} is not registered`)

  const adminStore = readFileSync(
    join(repoRoot, "stores/store-postgres/src/admin.rs"),
    "utf8"
  )
  const keyStore = readFileSync(
    join(repoRoot, "stores/store-postgres/src/lib.rs"),
    "utf8"
  )
  assert.match(adminStore, /FROM virtual_keys WHERE[\s\S]*project_id=\$3/)
  assert.match(
    adminStore,
    /OperationalView::ProviderHealth[\s\S]*provider_health\(self, query\)/
  )
  assert.match(
    adminStore,
    /u\.project_id=\$2[\s\S]*u\.provider=\$5[\s\S]*upstream_model=\$6/
  )
  assert.match(
    keyStore,
    /tenant_id = \$1 AND id = \$2 AND \(\$3::text IS NULL OR project_id = \$3\)/
  )
})
