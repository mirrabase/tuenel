const text = (data, name) => String(data.get(name) ?? "").trim()

const required = (data, name, label = name) => {
  const value = text(data, name)
  if (!value) throw new Error(`${label} is required`)
  return value
}

const optionalNumber = (data, name, { integer = false } = {}) => {
  const value = text(data, name)
  if (!value) return undefined
  const number = Number(value)
  if (
    !Number.isFinite(number) ||
    number < 0 ||
    (integer && !Number.isInteger(number))
  )
    throw new Error(
      `${name.replaceAll("_", " ")} must be a non-negative number`
    )
  return number
}

const positiveInteger = (data, name, label = name) => {
  const value = optionalNumber(data, name, { integer: true })
  if (!value) throw new Error(`${label} must be greater than zero`)
  return value
}

const requiredNumber = (data, name, label = name) => {
  const value = optionalNumber(data, name)
  if (value === undefined) throw new Error(`${label} is required`)
  return value
}

const optionalIso = (data, name) => {
  const value = text(data, name)
  if (!value) return undefined
  const date = new Date(value)
  if (Number.isNaN(date.valueOf()))
    throw new Error(`${name.replaceAll("_", " ")} is invalid`)
  return date.toISOString()
}

const list = (data, name) =>
  text(data, name)
    .split(",")
    .map((value) => value.trim())
    .filter(Boolean)

const compact = (value) =>
  Object.fromEntries(
    Object.entries(value).filter(([, item]) => item !== undefined)
  )

export function buildResourcePayload(
  kind,
  data,
  tenantId,
  editing = false,
  projectId
) {
  switch (kind) {
    case "projects":
      return compact({
        name: required(data, "name", "Name"),
        tenant_id: editing ? undefined : tenantId,
      })
    case "providers": {
      const providerType = required(data, "provider_type", "Provider type")
      const credential = text(data, "credential")
      if (!editing && providerType !== "openai_compatible" && !credential)
        throw new Error("Credential is required for this provider")
      return compact({
        id: editing ? undefined : required(data, "id", "Provider ID"),
        name: required(data, "name", "Name"),
        provider_type: providerType,
        base_url:
          providerType === "openai"
            ? "https://api.openai.com/v1/"
            : required(data, "base_url", "Base URL"),
        credential: credential || undefined,
      })
    }
    case "routing":
      return compact({
        tenant_id: projectId && !editing ? tenantId : undefined,
        project_id: projectId,
        provider: required(data, "provider", "Provider"),
        requested_model: required(data, "requested_model", "Requested model"),
        upstream_model: required(data, "upstream_model", "Upstream model"),
        priority: positiveInteger(data, "priority", "Priority"),
        enabled: data.get("enabled") === "on",
      })
    case "pricing": {
      const effectiveFrom = optionalIso(data, "effective_from")
      const effectiveUntil = optionalIso(data, "effective_until")
      if (!effectiveFrom) throw new Error("Effective from is required")
      if (effectiveUntil && effectiveUntil <= effectiveFrom)
        throw new Error("Effective until must be after effective from")
      return compact({
        provider_id: required(data, "provider_id", "Provider"),
        upstream_model: required(data, "upstream_model", "Upstream model"),
        input_cost_per_million: requiredNumber(
          data,
          "input_cost_per_million",
          "Input cost"
        ),
        output_cost_per_million: requiredNumber(
          data,
          "output_cost_per_million",
          "Output cost"
        ),
        cached_input_cost_per_million: optionalNumber(
          data,
          "cached_input_cost_per_million"
        ),
        embedding_cost_per_million: optionalNumber(
          data,
          "embedding_cost_per_million"
        ),
        effective_from: effectiveFrom,
        effective_until: effectiveUntil,
      })
    }
    case "policies":
      return compact({
        tenant_id: editing ? undefined : tenantId,
        scope_kind: required(data, "scope_kind", "Scope"),
        scope_id: required(data, "scope_id", "Scope ID"),
        policy: compact({
          allowed_models: list(data, "allowed_models"),
          denied_models: list(data, "denied_models"),
          allowed_operations: list(data, "allowed_operations"),
          max_output_tokens: optionalNumber(data, "max_output_tokens", {
            integer: true,
          }),
          concurrent_requests: optionalNumber(data, "concurrent_requests", {
            integer: true,
          }),
          daily_token_limit: optionalNumber(data, "daily_token_limit", {
            integer: true,
          }),
          monthly_token_limit: optionalNumber(data, "monthly_token_limit", {
            integer: true,
          }),
        }),
      })
    case "quotas": {
      const limits = compact({
        token_limit: optionalNumber(data, "token_limit", { integer: true }),
        cost_limit: optionalNumber(data, "cost_limit"),
        concurrent_limit: optionalNumber(data, "concurrent_limit", {
          integer: true,
        }),
        requests_per_minute: optionalNumber(data, "requests_per_minute", {
          integer: true,
        }),
      })
      if (!Object.keys(limits).length)
        throw new Error("Enter at least one quota limit")
      if (Object.values(limits).some((value) => value <= 0))
        throw new Error("Quota limits must be greater than zero")
      return compact({
        tenant_id: editing ? undefined : tenantId,
        scope_kind: required(data, "scope_kind", "Scope"),
        scope_id: required(data, "scope_id", "Scope ID"),
        period: required(data, "period", "Period"),
        ...limits,
      })
    }
    default:
      throw new Error("Unsupported resource")
  }
}
