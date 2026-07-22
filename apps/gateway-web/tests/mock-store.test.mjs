import assert from "node:assert/strict"
import { test } from "node:test"

import { canAccessOperator, createSeedState, mockReducer, visibleApprovals } from "../lib/mock-store.ts"

const reduce = (state, action) => mockReducer(state, action)
const login = (role = "gateway_admin") => reduce(createSeedState(), { type: "login", mode: "oidc", role })

test("mock login, RBAC, context switching, and tenant isolation", () => {
  const tenant = login("tenant_user")
  assert.equal(canAccessOperator(tenant), false)
  assert.equal(reduce(tenant, { type: "switch-context", tenantId: "northstar", projectId: "northstar-prod" }), tenant)
  assert.deepEqual(visibleApprovals(tenant).map((item) => item.tenantId), ["acme"])
  assert.equal(reduce(tenant, { type: "key.revoke", id: "key_demo_2" }).keys.key_demo_2.status, "Active")

  const admin = login()
  assert.equal(reduce(admin, { type: "approval.decide", id: "apr-demo-2", status: "approved", reason: "cross-tenant" }), admin)
  const switched = reduce(admin, { type: "switch-context", tenantId: "northstar", projectId: "northstar-prod" })
  assert.equal(switched.principal.tenantId, "northstar")
  assert.deepEqual(visibleApprovals(switched).map((item) => item.tenantId), ["northstar"])
})

test("MCP CRUD, health, refresh discovery, and policy lifecycle", () => {
  let state = login()
  state = reduce(state, { type: "server.create", server: { tenantId: "acme", name: "Demo", transport: "http", endpoint: "https://demo.invalid", enabled: true } })
  const server = Object.values(state.servers).find((item) => item.name === "Demo")
  assert.equal(server.health, "degraded")
  state = reduce(state, { type: "server.health", id: server.id })
  state = reduce(state, { type: "server.refresh", id: server.id })
  assert.equal(state.servers[server.id].health, "healthy")
  assert.equal(state.servers[server.id].toolIds.length, 1)
  state = reduce(state, { type: "server.update", id: server.id, patch: { name: "Updated demo" } })
  assert.equal(state.servers[server.id].name, "Updated demo")
  const policy = { ...Object.values(state.policies)[0], id: "pol-test", name: "Test" }
  state = reduce(state, { type: "policy.save", policy })
  assert.equal(state.policies[policy.id].name, "Test")
  state = reduce(state, { type: "policy.save", policy: { ...policy, name: "Updated" } })
  assert.equal(state.policies[policy.id].name, "Updated")
  state = reduce(state, { type: "policy.delete", id: policy.id })
  state = reduce(state, { type: "server.delete", id: server.id })
  assert.equal(state.policies[policy.id], undefined)
  assert.equal(state.servers[server.id], undefined)
})

test("approval pending transitions to approved, rejected, or expired", () => {
  const seed = login()
  assert.equal(reduce(seed, { type: "approval.decide", id: "apr-demo-1", status: "approved", reason: "ok" }).approvals["apr-demo-1"].status, "approved")
  assert.equal(reduce(seed, { type: "approval.decide", id: "apr-demo-1", status: "rejected", reason: "no" }).approvals["apr-demo-1"].status, "rejected")
  assert.equal(reduce(seed, { type: "approval.expire", id: "apr-demo-1" }).approvals["apr-demo-1"].status, "expired")
})

test("approved invocation returns exactly one result across idempotent retries", () => {
  let state = login()
  const invoke = { type: "invoke", tool: "delete_file", scenario: "approval", idempotencyKey: "idem-approval" }
  state = reduce(state, invoke)
  const approvalId = state.invocations["idem-approval"].approvalId
  state = reduce(state, { type: "approval.decide", id: approvalId, status: "approved", reason: "reviewed" })
  state = reduce(state, invoke)
  assert.equal(state.invocations["idem-approval"].status, "succeeded")
  const result = state.invocations["idem-approval"].result
  const retried = reduce(state, invoke)
  assert.equal(retried.invocations["idem-approval"].result, result)
  assert.equal(Object.keys(retried.invocations).length, 1)
})

test("credential/injection blocks, PII redacts, and malicious MCP results block", () => {
  let state = login()
  state = reduce(state, { type: "invoke", tool: "search_files", scenario: "block", idempotencyKey: "secret-injection" })
  state = reduce(state, { type: "invoke", tool: "search_files", scenario: "redact", idempotencyKey: "pii" })
  state = reduce(state, { type: "invoke", tool: "search_files", scenario: "malicious-result", idempotencyKey: "malicious" })
  assert.equal(state.invocations["secret-injection"].status, "blocked")
  assert.match(state.invocations.pii.result, /\[EMAIL\]/)
  assert.equal(state.invocations.malicious.status, "blocked")
})

test("incident transitions and Virtual Key shown-once/revoke", () => {
  let state = login()
  state = reduce(state, { type: "incident.status", id: "inc-demo-1", status: "investigating", note: "Sanitized note" })
  assert.equal(state.incidents["inc-demo-1"].status, "investigating")
  state = reduce(state, { type: "key.issue", name: "CI demo" })
  assert.match(state.revealedSecret, /^vk_demo_/)
  const key = Object.values(state.keys).find((item) => item.name === "CI demo")
  state = reduce(state, { type: "key.clear-secret" })
  state = reduce(state, { type: "key.revoke", id: key.id })
  assert.equal(state.revealedSecret, null)
  assert.equal(state.keys[key.id].status, "Revoked")
})
