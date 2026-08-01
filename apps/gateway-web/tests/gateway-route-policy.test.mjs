import assert from "node:assert/strict"
import test from "node:test"

import { allowedGatewayRoute } from "../lib/gateway-route-policy.ts"

const tenant = "018f3f1a-9b2c-7def-8123-456789abcdef"
const otherTenant = "018f3f1a-9b2c-7def-8123-456789abcdee"

test("managed commercial routes allow only their intended methods", () => {
  assert.equal(
    allowedGatewayRoute(
      `/commercial/tenants/${tenant}/billing/status`,
      "GET",
      tenant
    ),
    true
  )
  assert.equal(
    allowedGatewayRoute(
      `/commercial/tenants/${tenant}/billing/checkout`,
      "POST",
      tenant
    ),
    true
  )
  assert.equal(
    allowedGatewayRoute(
      `/commercial/tenants/${tenant}/billing/subscription`,
      "PATCH",
      tenant
    ),
    true
  )
  for (const route of [
    "billing/trial/start",
    "billing/subscription/cancel",
    "billing/subscription/resume",
  ]) {
    assert.equal(
      allowedGatewayRoute(`/commercial/tenants/${tenant}/${route}`, "POST", tenant),
      true
    )
  }
  assert.equal(
    allowedGatewayRoute("/commercial/billing/catalog", "GET", tenant),
    true
  )
  assert.equal(
    allowedGatewayRoute(`/commercial/tenants/${tenant}/oidc`, "PUT", tenant),
    true
  )
  assert.equal(
    allowedGatewayRoute(
      `/commercial/tenants/${tenant}/billing/status`,
      "POST",
      tenant
    ),
    false
  )
})

test("commercial routes reject arbitrary paths and cross-tenant scope", () => {
  assert.equal(
    allowedGatewayRoute(
      `/commercial/tenants/${tenant}/license/activate`,
      "POST",
      tenant
    ),
    false
  )
  assert.equal(
    allowedGatewayRoute(
      `/commercial/tenants/${otherTenant}/billing/status`,
      "GET",
      tenant
    ),
    false
  )
  assert.equal(
    allowedGatewayRoute("/commercial/license/status", "GET", tenant),
    false
  )
})
