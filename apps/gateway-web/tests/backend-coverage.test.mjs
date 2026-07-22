import assert from "node:assert/strict"
import { existsSync, readFileSync } from "node:fs"
import { join } from "node:path"
import { test } from "node:test"

const webRoot = new URL("..", import.meta.url).pathname.replace(/^\/(.:)/, "$1")
const repoRoot = join(webRoot, "..", "..")
const backend = [
  "crates/gateway-server/src/lib.rs",
  "crates/gateway-server/src/v03.rs",
]
  .map((file) => readFileSync(join(repoRoot, file), "utf8"))
  .join("\n")
const backendRoutes = [...backend.matchAll(/\.route\(\s*"([^"]+)"/g)].map(
  (match) => match[1]
)

const coverage = {
  "/auth/signup": "/[locale]/(auth)/register",
  "/auth/login": "/[locale]/(auth)/login",
  "/auth/session": "/[locale]/(auth)/login",
  "/auth/tenants/{tenant_id}/invitations": "/[locale]/[tenantId]/[[...slug]]",
  "/auth/invitations/accept": "/[locale]/(auth)/invite",
  "/health": "/docs",
  "/ready": "/docs",
  "/metrics": "/docs",
  "/openapi.json": "/docs",
  "/v1/models": "/models",
  "/v1/chat/completions": "/playground",
  "/v1/responses": "/playground",
  "/v1/embeddings": "/playground",
  "/admin/virtual-keys": "/keys",
  "/admin/virtual-keys/{id}": "/keys",
  "/admin/mcp/servers": "/operator/mcp",
  "/admin/mcp/servers/{server_id}": "/operator/mcp",
  "/admin/mcp/servers/{server_id}/refresh": "/operator/mcp",
  "/admin/mcp/servers/{server_id}/health": "/operator/mcp",
  "/admin/mcp/servers/{server_id}/tools": "/operator/mcp",
  "/admin/mcp/tools": "/operator/mcp",
  "/admin/mcp/policies": "/operator/mcp/policies",
  "/admin/mcp/policies/{policy_id}": "/operator/mcp/policies",
  "/v1/mcp/servers": "/mcp",
  "/v1/mcp/tools": "/mcp",
  "/v1/mcp/tools/call": "/mcp",
  "/admin/approvals": "/operator/approvals",
  "/admin/approvals/{approval_id}": "/operator/approvals",
  "/admin/approvals/{approval_id}/approve": "/operator/approvals",
  "/admin/approvals/{approval_id}/reject": "/operator/approvals",
  "/v1/gateway/approvals/{approval_id}": "/mcp",
  "/admin/security/policies": "/operator/security/policies",
  "/admin/security/policies/{policy_id}": "/operator/security/policies",
  "/admin/security/incidents": "/operator/security",
  "/admin/security/incidents/{incident_id}": "/operator/security",
  "/admin/security/findings": "/operator/security",
  "/admin/security/events": "/operator/security",
  "/mcp": "machine-only",
}

test("every backend HTTP route maps to a UI surface or documented machine-only exemption", () => {
  const gaps = readFileSync(
    join(repoRoot, "GATEWAY_WEB_BACKEND_GAPS.md"),
    "utf8"
  )
  assert.ok(backendRoutes.length > 20)
  for (const route of backendRoutes) {
    assert.ok(coverage[route], `missing coverage classification for ${route}`)
    if (coverage[route] === "machine-only") assert.match(gaps, /Native `\/mcp`/)
    else {
      const relative =
        coverage[route] === "/"
          ? "app/page.tsx"
          : `app${coverage[route]}/page.tsx`
      assert.equal(
        existsSync(join(webRoot, relative)),
        true,
        `${route} maps to missing ${relative}`
      )
    }
  }
})
