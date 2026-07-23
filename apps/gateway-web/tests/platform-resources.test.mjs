import assert from "node:assert/strict"
import { test } from "node:test"

import { buildResourcePayload } from "../lib/platform-resources.js"

const tenantId = "01900000-0000-7000-8000-000000000001"
const form = (values) => {
  const data = new FormData()
  for (const [key, value] of Object.entries(values))
    data.set(key, String(value))
  return data
}

test("dedicated forms build backend-compatible payloads", () => {
  assert.deepEqual(
    buildResourcePayload("projects", form({ name: "Core" }), tenantId),
    { name: "Core", tenant_id: tenantId }
  )

  assert.deepEqual(
    buildResourcePayload(
      "providers",
      form({
        id: "openai-main",
        name: "OpenAI",
        provider_type: "openai_compatible",
        base_url: "https://api.openai.com/v1",
      }),
      tenantId
    ),
    {
      id: "openai-main",
      name: "OpenAI",
      provider_type: "openai_compatible",
      base_url: "https://api.openai.com/v1",
    }
  )

  assert.deepEqual(
    buildResourcePayload(
      "routing",
      form({
        provider: "openai-main",
        requested_model: "gateway-model",
        upstream_model: "gpt-5",
        priority: 2,
        enabled: "on",
      }),
      tenantId
    ),
    {
      provider: "openai-main",
      requested_model: "gateway-model",
      upstream_model: "gpt-5",
      priority: 2,
      enabled: true,
    }
  )

  const price = buildResourcePayload(
    "pricing",
    form({
      provider_id: "openai-main",
      upstream_model: "gpt-5",
      input_cost_per_million: 1.25,
      output_cost_per_million: 10,
      effective_from: "2026-07-23T10:00",
    }),
    tenantId
  )
  assert.equal(price.input_cost_per_million, 1.25)
  assert.match(price.effective_from, /^2026-07-23T/)

  assert.deepEqual(
    buildResourcePayload(
      "policies",
      form({
        scope_kind: "tenant",
        scope_id: tenantId,
        allowed_models: "gpt-5, claude-sonnet, ",
        denied_models: "",
        allowed_operations: "chat, responses",
        max_output_tokens: 4096,
      }),
      tenantId
    ),
    {
      tenant_id: tenantId,
      scope_kind: "tenant",
      scope_id: tenantId,
      policy: {
        allowed_models: ["gpt-5", "claude-sonnet"],
        denied_models: [],
        allowed_operations: ["chat", "responses"],
        max_output_tokens: 4096,
      },
    }
  )

  assert.deepEqual(
    buildResourcePayload(
      "quotas",
      form({
        scope_kind: "tenant",
        scope_id: tenantId,
        period: "day",
        token_limit: 100000,
      }),
      tenantId
    ),
    {
      tenant_id: tenantId,
      scope_kind: "tenant",
      scope_id: tenantId,
      period: "day",
      token_limit: 100000,
    }
  )
})

test("edit payloads preserve provider secrets and resource tenant scope", () => {
  const provider = buildResourcePayload(
    "providers",
    form({
      id: "ignored",
      name: "OpenAI",
      provider_type: "openai_compatible",
      base_url: "https://api.openai.com/v1",
      credential: "",
    }),
    tenantId,
    true
  )
  assert.equal("id" in provider, false)
  assert.equal("credential" in provider, false)

  const project = buildResourcePayload(
    "projects",
    form({ name: "Renamed" }),
    tenantId,
    true
  )
  assert.equal("tenant_id" in project, false)
})

test("unsafe incomplete payloads are rejected before the API call", () => {
  assert.throws(
    () =>
      buildResourcePayload(
        "providers",
        form({
          id: "anthropic",
          name: "Anthropic",
          provider_type: "anthropic",
          base_url: "https://api.anthropic.com",
        }),
        tenantId
      ),
    /Credential is required/
  )
  assert.throws(
    () =>
      buildResourcePayload(
        "quotas",
        form({
          scope_kind: "tenant",
          scope_id: tenantId,
          period: "day",
        }),
        tenantId
      ),
    /at least one quota limit/
  )
})
