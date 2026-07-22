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
    "app/[locale]/[tenantId]/layout.tsx",
    "app/[locale]/[tenantId]/[[...slug]]/page.tsx",
  ])
    assert.doesNotThrow(() => read(path))
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
  assert.match(bff, /headers\.delete\("authorization"\)/)
  assert.match(bff, /Invalid request origin/)
  assert.match(bff, /Gateway path is not allowed/)
  assert.match(bff, /credential}\.\$\{tenant/)
})
