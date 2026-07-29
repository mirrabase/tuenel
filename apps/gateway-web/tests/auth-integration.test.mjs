import assert from "node:assert/strict"
import { readFileSync } from "node:fs"
import { test } from "node:test"
import { join } from "node:path"

const root = new URL("..", import.meta.url).pathname.replace(/^\/(.:)/, "$1")
const read = (path) => readFileSync(join(root, path), "utf8")

test("localized auth and tenant routes are present", () => {
  for (const path of [
    "app/[locale]/(auth)/login/page.tsx",
    "app/[locale]/(auth)/register/page.tsx",
    "app/[locale]/(auth)/invite/page.tsx",
    "app/[locale]/(auth)/setup/page.tsx",
    "app/[locale]/[tenantId]/layout.tsx",
    "app/[locale]/[tenantId]/[[...slug]]/page.tsx",
  ])
    assert.doesNotThrow(() => read(path))
})

test("bootstrap, verification, and invitation secrets use scrubbed URL fragments", () => {
  const setup = read("components/setup-form.tsx")
  const verify = read("components/verification-form.tsx")
  const invite = read("app/[locale]/(auth)/invite/page.tsx")
  const members = read("components/pages/members-page.tsx")
  for (const source of [setup, verify, invite]) {
    assert.match(source, /window\.location\.hash/)
    assert.match(source, /history\.replaceState/)
  }
  assert.match(members, /invite#token=/)
  assert.doesNotMatch(members, /invite\?token=/)
  assert.match(invite, /disabled=\{!token \|\| pending\}/)
})

test("auth capabilities gate public signup and bootstrap sets a hardened session", () => {
  const login = read("app/[locale]/(auth)/login/page.tsx")
  const register = read("app/[locale]/(auth)/register/page.tsx")
  const route = read("app/api/auth/[action]/route.ts")
  assert.match(login, /bootstrap_required/)
  assert.match(register, /registration_mode !== "public"/)
  assert.match(route, /"bootstrap"/)
  assert.match(route, /"invitation-register"/)
  assert.match(route, /hasValidOrigin/)
})

test("tenant capabilities are fetched server-side and passed as presentation state", () => {
  const session = read("lib/server-auth.ts")
  const layout = read("app/[locale]/[tenantId]/layout.tsx")
  const provider = read("components/gateway-provider.tsx")
  assert.match(session, /getTenantCapabilities/)
  assert.match(
    session,
    /\/auth\/tenants\/\$\{encodeURIComponent\(tenantId\)\}\/capabilities/
  )
  assert.match(layout, /getTenantCapabilities\(tenantId\)/)
  assert.match(layout, /browserSso:/)
  assert.match(layout, /auditExport:/)
  assert.match(provider, /edition: "community" \| "enterprise" \| "managed"/)
})

test("session cookie is encrypted and hardened", () => {
  const session = read("lib/server-auth.ts")
  const authRoute = read("app/api/auth/[action]/route.ts")
  assert.match(session, /AES-GCM/)
  assert.match(authRoute, /httpOnly:\s*true/)
  assert.match(authRoute, /sameSite:\s*"lax"/)
  assert.doesNotMatch(authRoute, /localStorage|sessionStorage/)
})

test("gateway BFF strips caller credentials and allowlists paths", () => {
  const bff = read("app/api/gateway/[...path]/route.ts")
  assert.match(bff, /const headers = new Headers\(\)/)
  assert.match(bff, /for \(const name of \[/)
  assert.match(bff, /Invalid request origin/)
  assert.match(bff, /Gateway path is not allowed/)
  assert.match(bff, /credential}\.\$\{tenant/)
})

test("successful login returns through the localized tenant redirect", () => {
  const form = read("components/auth-form.tsx")
  assert.match(form, /router\.replace\(`\/\$\{locale}`\)/)
  assert.doesNotMatch(form, /router\.replace\(mode === "signup"/)
})
