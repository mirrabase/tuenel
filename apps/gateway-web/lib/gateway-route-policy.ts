const rules: [RegExp, ReadonlySet<string>][] = [
  [/^\/(health|ready|openapi\.json)$/, new Set(["GET"])],
  [
    /^\/auth\/(tenants|invitations)(\/|$)/,
    new Set(["GET", "POST", "PATCH", "DELETE"]),
  ],
  [
    /^\/v1\/(models|chat\/completions|responses|embeddings)$/,
    new Set(["GET", "POST"]),
  ],
  [/^\/v1\/(mcp|gateway\/approvals)\//, new Set(["GET", "POST"])],
  [/^\/admin\//, new Set(["GET", "POST", "PATCH", "DELETE"])],
]

const commercialTenantRoute =
  /^\/commercial\/tenants\/([0-9a-f-]{36})\/(billing\/(?:checkout|portal|status|subscription|trial\/start|subscription\/(?:cancel|resume))|oidc|audit\/export)$/i

const commercialMethods: Record<string, ReadonlySet<string>> = {
  "billing/checkout": new Set(["POST"]),
  "billing/portal": new Set(["POST"]),
  "billing/status": new Set(["GET"]),
  "billing/subscription": new Set(["PATCH"]),
  "billing/trial/start": new Set(["POST"]),
  "billing/subscription/cancel": new Set(["POST"]),
  "billing/subscription/resume": new Set(["POST"]),
  oidc: new Set(["GET", "PUT"]),
  "audit/export": new Set(["GET"]),
}

export function allowedGatewayRoute(
  path: string,
  method: string,
  scopedTenant: string
) {
  if (path === "/commercial/billing/catalog" && method === "GET") return true
  const commercial = commercialTenantRoute.exec(path)
  if (commercial) {
    const [, pathTenant, route] = commercial
    return (
      pathTenant.toLowerCase() === scopedTenant.toLowerCase() &&
      commercialMethods[route]?.has(method) === true
    )
  }
  return rules.some(
    ([pattern, methods]) => pattern.test(path) && methods.has(method)
  )
}
