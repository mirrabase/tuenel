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
