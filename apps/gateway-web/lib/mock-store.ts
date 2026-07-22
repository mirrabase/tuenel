export type Role = "tenant_user" | "gateway_admin"
export type ApprovalStatus = "pending" | "approved" | "rejected" | "expired"
export type SecurityAction = "allow" | "warn" | "redact" | "block"

export type Principal = {
  id: string
  name: string
  email: string
  role: Role
  tenantRole?: "owner" | "admin" | "engineer" | "viewer"
  tenantId: string
  projectId: string
  authMode: "oidc" | "development-token"
}

export type VirtualKey = {
  id: string
  tenantId: string
  name: string
  prefix: string
  status: "Active" | "Revoked"
  quota: string
}

export type McpServer = {
  id: string
  tenantId: string
  name: string
  transport: "http" | "stdio"
  endpoint?: string
  command?: string
  enabled: boolean
  health: "healthy" | "degraded" | "disabled"
  refreshedAt: string
  toolIds: string[]
}

export type McpTool = {
  id: string
  serverId: string
  name: string
  description: string
  risk: "low" | "medium" | "high"
  schema: Record<string, unknown>
}

export type McpPolicy = {
  id: string
  name: string
  tenantId: string
  projectId?: string
  serverRule: "allow" | "deny"
  toolRule: "allow" | "deny"
  action: "allow" | "warn" | "approval" | "block"
  riskOverride: "inherit" | "low" | "medium" | "high"
  argumentRule: string
  rpm: number
  daily: number
  concurrency: number
  maxBytes: number
  timeoutMs: number
}

export type Approval = {
  id: string
  tenantId: string
  requestId: string
  tool: string
  summary: string
  status: ApprovalStatus
  expiresAt: string
  reason?: string
}

export type Invocation = {
  id: string
  tenantId: string
  tool: string
  idempotencyKey: string
  scenario:
    "safe" | "warn" | "redact" | "block" | "approval" | "malicious-result"
  status: "succeeded" | "warned" | "redacted" | "blocked" | "approval_required"
  result?: string
  approvalId?: string
}

export type SecurityPolicy = {
  id: string
  name: string
  inspectRequest: boolean
  inspectResponse: boolean
  inspectMcpArguments: boolean
  inspectMcpResult: boolean
  failOpen: boolean
  createIncident: boolean
  maxBytes: number
  matrix: Record<
    "credentials" | "injection" | "pii" | "malware",
    SecurityAction
  >
}

export type Incident = {
  id: string
  tenantId: string
  title: string
  severity: "low" | "medium" | "high" | "critical"
  status: "open" | "investigating" | "resolved"
  requestId: string
  riskScore: number
  note: string
}

export type MockState = {
  principal: Principal | null
  tenants: Record<string, { id: string; name: string }>
  projects: Record<string, { id: string; tenantId: string; name: string }>
  requests: Array<{
    id: string
    tenantId: string
    surface: "chat" | "responses" | "embeddings"
    status: string
  }>
  usage: Array<{
    requestId: string
    tenantId: string
    tokens: number
    cost: string
  }>
  quota: Record<string, { limit: number; used: number }>
  providers: Array<{ id: string; status: string; latency: string }>
  routing: Array<{ alias: string; providerId: string }>
  pricing: Array<{ model: string; input: string; output: string }>
  billing: Array<{ id: string; status: string; attempts: number }>
  audit: Array<{ id: string; actor: string; action: string; resource: string }>
  keys: Record<string, VirtualKey>
  revealedSecret: string | null
  servers: Record<string, McpServer>
  tools: Record<string, McpTool>
  policies: Record<string, McpPolicy>
  approvals: Record<string, Approval>
  invocations: Record<string, Invocation>
  securityPolicies: Record<string, SecurityPolicy>
  incidents: Record<string, Incident>
  findings: Array<{
    id: string
    tenantId: string
    category: string
    evidence: string
    severity: string
  }>
  securityEvents: Array<{
    id: string
    tenantId: string
    requestId: string
    action: SecurityAction
    detail: string
  }>
}

export type MockAction =
  | { type: "login"; mode: Principal["authMode"]; role: Role }
  | { type: "logout" }
  | { type: "switch-context"; tenantId: string; projectId: string }
  | { type: "reset" }
  | { type: "key.issue"; name: string }
  | { type: "key.clear-secret" }
  | { type: "key.revoke"; id: string }
  | {
      type: "server.create"
      server: Omit<McpServer, "id" | "health" | "refreshedAt" | "toolIds">
    }
  | { type: "server.update"; id: string; patch: Partial<McpServer> }
  | { type: "server.delete"; id: string }
  | { type: "server.health"; id: string }
  | { type: "server.refresh"; id: string }
  | { type: "policy.save"; policy: McpPolicy }
  | { type: "policy.delete"; id: string }
  | {
      type: "approval.decide"
      id: string
      status: "approved" | "rejected"
      reason: string
    }
  | { type: "approval.expire"; id: string }
  | {
      type: "invoke"
      tool: string
      scenario: Invocation["scenario"]
      idempotencyKey: string
    }
  | {
      type: "incident.status"
      id: string
      status: Incident["status"]
      note: string
    }
  | { type: "security-policy.save"; policy: SecurityPolicy }

const tenants = {
  acme: { id: "acme", name: "Acme Production" },
  northstar: { id: "northstar", name: "Northstar Labs" },
}

const projects = {
  "acme-prod": { id: "acme-prod", tenantId: "acme", name: "Production API" },
  "acme-eval": { id: "acme-eval", tenantId: "acme", name: "Evaluation" },
  "northstar-prod": {
    id: "northstar-prod",
    tenantId: "northstar",
    name: "Production",
  },
}

export function createSeedState(): MockState {
  return {
    principal: null,
    tenants,
    projects,
    requests: [
      {
        id: "req-demo-chat",
        tenantId: "acme",
        surface: "chat",
        status: "succeeded",
      },
      {
        id: "req-demo-embed",
        tenantId: "acme",
        surface: "embeddings",
        status: "redacted",
      },
    ],
    usage: [
      {
        requestId: "req-demo-chat",
        tenantId: "acme",
        tokens: 2841,
        cost: "$0.0184",
      },
    ],
    quota: {
      acme: { limit: 1_000_000, used: 678_000 },
      northstar: { limit: 500_000, used: 455_000 },
    },
    providers: [
      { id: "openai-compatible", status: "healthy", latency: "1.8s" },
    ],
    routing: [{ alias: "gateway-default", providerId: "openai-compatible" }],
    pricing: [{ model: "gateway-default", input: "$0.40", output: "$1.60" }],
    billing: [{ id: "bill-demo-1", status: "delivered", attempts: 1 }],
    audit: [
      {
        id: "audit-demo-1",
        actor: "usr-admin",
        action: "policy.updated",
        resource: "pol-mcp-1",
      },
    ],
    revealedSecret: null,
    keys: {
      key_demo_1: {
        id: "key_demo_1",
        tenantId: "acme",
        name: "Production API",
        prefix: "vk_demo_7f3a••••",
        status: "Active",
        quota: "100k/day",
      },
      key_demo_2: {
        id: "key_demo_2",
        tenantId: "northstar",
        name: "Northstar worker",
        prefix: "vk_demo_91bd••••",
        status: "Active",
        quota: "25k/day",
      },
    },
    servers: {
      "srv-files": {
        id: "srv-files",
        tenantId: "acme",
        name: "Files API",
        transport: "http",
        endpoint: "https://mcp.demo.invalid",
        enabled: true,
        health: "healthy",
        refreshedAt: "2026-07-22T08:00:00Z",
        toolIds: ["tool-search", "tool-delete"],
      },
      "srv-reports": {
        id: "srv-reports",
        tenantId: "northstar",
        name: "Report runner",
        transport: "stdio",
        command: "demo-report-server",
        enabled: true,
        health: "degraded",
        refreshedAt: "2026-07-22T07:30:00Z",
        toolIds: ["tool-report"],
      },
    },
    tools: {
      "tool-search": {
        id: "tool-search",
        serverId: "srv-files",
        name: "search_files",
        description: "Search permitted demo files.",
        risk: "low",
        schema: {
          type: "object",
          properties: { query: { type: "string" } },
          required: ["query"],
        },
      },
      "tool-delete": {
        id: "tool-delete",
        serverId: "srv-files",
        name: "delete_file",
        description: "Delete one demo file.",
        risk: "high",
        schema: {
          type: "object",
          properties: { path: { type: "string" } },
          required: ["path"],
        },
      },
      "tool-report": {
        id: "tool-report",
        serverId: "srv-reports",
        name: "run_report",
        description: "Create a sanitized report.",
        risk: "medium",
        schema: { type: "object", properties: { period: { type: "string" } } },
      },
    },
    policies: {
      "pol-mcp-1": {
        id: "pol-mcp-1",
        name: "Production safe tools",
        tenantId: "acme",
        projectId: "acme-prod",
        serverRule: "allow",
        toolRule: "allow",
        action: "approval",
        riskOverride: "inherit",
        argumentRule: "path must start with /reports",
        rpm: 30,
        daily: 500,
        concurrency: 3,
        maxBytes: 65536,
        timeoutMs: 15000,
      },
    },
    approvals: {
      "apr-demo-1": {
        id: "apr-demo-1",
        tenantId: "acme",
        requestId: "req-demo-approval",
        tool: "delete_file",
        summary: "Delete /reports/old.csv (arguments sanitized)",
        status: "pending",
        expiresAt: "2026-07-22T10:30:00Z",
      },
      "apr-demo-2": {
        id: "apr-demo-2",
        tenantId: "northstar",
        requestId: "req-northstar",
        tool: "run_report",
        summary: "Northstar-only request",
        status: "pending",
        expiresAt: "2026-07-22T10:45:00Z",
      },
    },
    invocations: {},
    securityPolicies: {
      "sec-default": {
        id: "sec-default",
        name: "Default inspection",
        inspectRequest: true,
        inspectResponse: true,
        inspectMcpArguments: true,
        inspectMcpResult: true,
        failOpen: false,
        createIncident: true,
        maxBytes: 1048576,
        matrix: {
          credentials: "block",
          injection: "block",
          pii: "redact",
          malware: "block",
        },
      },
    },
    incidents: {
      "inc-demo-1": {
        id: "inc-demo-1",
        tenantId: "acme",
        title: "Prompt injection blocked",
        severity: "high",
        status: "open",
        requestId: "req-demo-injection",
        riskScore: 88,
        note: "Sanitized evidence retained; raw prompt omitted.",
      },
    },
    findings: [
      {
        id: "find-1",
        tenantId: "acme",
        category: "prompt_injection",
        evidence: "Instruction override pattern [redacted]",
        severity: "high",
      },
      {
        id: "find-2",
        tenantId: "acme",
        category: "pii",
        evidence: "Email address replaced with [EMAIL]",
        severity: "medium",
      },
    ],
    securityEvents: [
      {
        id: "sev-1",
        tenantId: "acme",
        requestId: "req-demo-injection",
        action: "block",
        detail: "Request stopped before provider execution.",
      },
      {
        id: "sev-2",
        tenantId: "acme",
        requestId: "req-demo-pii",
        action: "redact",
        detail: "Response returned with deterministic demo redaction.",
      },
    ],
  }
}

const clone = (state: MockState): MockState => structuredClone(state)
const nextId = (prefix: string, size: number) => `${prefix}${size + 1}`

export function canAccessOperator(state: MockState) {
  return state.principal?.role === "gateway_admin"
}

export function visibleApprovals(state: MockState) {
  if (!state.principal) return []
  return Object.values(state.approvals).filter(
    (approval) => approval.tenantId === state.principal?.tenantId
  )
}

export function mockReducer(state: MockState, action: MockAction): MockState {
  if (action.type === "reset") return createSeedState()
  const next = clone(state)
  switch (action.type) {
    case "login":
      next.principal = {
        id: action.role === "gateway_admin" ? "usr-admin" : "usr-tenant",
        name:
          action.role === "gateway_admin" ? "Alan Operator" : "Avery Tenant",
        email:
          action.role === "gateway_admin"
            ? "alan@demo.invalid"
            : "avery@demo.invalid",
        role: action.role,
        tenantId: "acme",
        projectId: "acme-prod",
        authMode: action.mode,
      }
      return next
    case "logout":
      next.principal = null
      next.revealedSecret = null
      return next
    case "switch-context":
      if (
        !canAccessOperator(state) ||
        next.projects[action.projectId]?.tenantId !== action.tenantId
      )
        return state
      next.principal = next.principal
        ? {
            ...next.principal,
            tenantId: action.tenantId,
            projectId: action.projectId,
          }
        : null
      return next
    case "key.issue": {
      if (!next.principal) return state
      const id = nextId("key_demo_", Object.keys(next.keys).length)
      const secret = `vk_demo_${id.replace("key_demo_", "").padStart(24, "0")}`
      next.keys[id] = {
        id,
        tenantId: next.principal.tenantId,
        name: action.name,
        prefix: `${secret.slice(0, 14)}••••`,
        status: "Active",
        quota: "100k/day",
      }
      next.revealedSecret = secret
      return next
    }
    case "key.clear-secret":
      next.revealedSecret = null
      return next
    case "key.revoke":
      if (next.keys[action.id]?.tenantId === next.principal?.tenantId)
        next.keys[action.id].status = "Revoked"
      return next
    case "server.create": {
      if (
        !canAccessOperator(state) ||
        action.server.tenantId !== state.principal?.tenantId
      )
        return state
      const id = nextId("srv-demo-", Object.keys(next.servers).length)
      next.servers[id] = {
        ...action.server,
        id,
        health: action.server.enabled ? "degraded" : "disabled",
        refreshedAt: "Never",
        toolIds: [],
      }
      return next
    }
    case "server.update":
      if (
        !canAccessOperator(state) ||
        next.servers[action.id]?.tenantId !== state.principal?.tenantId
      )
        return state
      if (next.servers[action.id])
        next.servers[action.id] = {
          ...next.servers[action.id],
          ...action.patch,
        }
      return next
    case "server.delete":
      if (
        !canAccessOperator(state) ||
        next.servers[action.id]?.tenantId !== state.principal?.tenantId
      )
        return state
      delete next.servers[action.id]
      return next
    case "server.health":
      if (
        !canAccessOperator(state) ||
        next.servers[action.id]?.tenantId !== state.principal?.tenantId
      )
        return state
      if (next.servers[action.id])
        next.servers[action.id].health = next.servers[action.id].enabled
          ? "healthy"
          : "disabled"
      return next
    case "server.refresh": {
      const server = next.servers[action.id]
      if (
        !canAccessOperator(state) ||
        !server ||
        server.tenantId !== state.principal?.tenantId
      )
        return state
      const toolId = `tool-discovered-${server.id}`
      if (!server.toolIds.includes(toolId)) {
        server.toolIds.push(toolId)
        next.tools[toolId] = {
          id: toolId,
          serverId: server.id,
          name: "discovered_demo_tool",
          description: "Deterministic discovery fixture.",
          risk: "medium",
          schema: { type: "object", additionalProperties: false },
        }
      }
      server.refreshedAt = "2026-07-22T09:00:00Z"
      return next
    }
    case "policy.save":
      if (
        !canAccessOperator(state) ||
        action.policy.tenantId !== state.principal?.tenantId
      )
        return state
      next.policies[action.policy.id] = action.policy
      return next
    case "policy.delete":
      if (
        !canAccessOperator(state) ||
        next.policies[action.id]?.tenantId !== state.principal?.tenantId
      )
        return state
      delete next.policies[action.id]
      return next
    case "approval.decide":
      if (
        !canAccessOperator(state) ||
        next.approvals[action.id]?.tenantId !== state.principal?.tenantId
      )
        return state
      if (next.approvals[action.id]?.status === "pending")
        Object.assign(next.approvals[action.id], {
          status: action.status,
          reason: action.reason,
        })
      return next
    case "approval.expire":
      if (
        !canAccessOperator(state) ||
        next.approvals[action.id]?.tenantId !== state.principal?.tenantId
      )
        return state
      if (next.approvals[action.id]?.status === "pending")
        next.approvals[action.id].status = "expired"
      return next
    case "invoke": {
      if (!next.principal) return state
      const permittedTool = Object.values(next.tools).find(
        (tool) =>
          tool.name === action.tool &&
          next.servers[tool.serverId]?.tenantId === next.principal?.tenantId &&
          next.servers[tool.serverId]?.enabled
      )
      if (!permittedTool) return state
      const existing = next.invocations[action.idempotencyKey]
      if (existing) {
        const approval = existing.approvalId
          ? next.approvals[existing.approvalId]
          : undefined
        if (
          existing.status === "approval_required" &&
          approval?.status === "approved"
        ) {
          existing.status = "succeeded"
          existing.result = "Simulated approved result delivered exactly once."
          return next
        }
        return state
      }
      const outcome = {
        safe: ["succeeded", "Simulated result: 3 permitted files matched."],
        warn: [
          "warned",
          "Simulated warning: result may contain untrusted instructions.",
        ],
        redact: ["redacted", "Simulated result: contact [EMAIL] was redacted."],
        block: [
          "blocked",
          "OpenAI-compatible error: security_policy_violation.",
        ],
        "malicious-result": [
          "blocked",
          "Simulated malicious MCP result blocked after tool execution.",
        ],
        approval: ["approval_required", undefined],
      } as const
      const [status, result] = outcome[action.scenario]
      const approvalId =
        action.scenario === "approval"
          ? nextId("apr-invoke-", Object.keys(next.approvals).length)
          : undefined
      if (approvalId)
        next.approvals[approvalId] = {
          id: approvalId,
          tenantId: next.principal.tenantId,
          requestId: `req-${action.idempotencyKey}`,
          tool: action.tool,
          summary: "High-risk simulated tool call (arguments sanitized)",
          status: "pending",
          expiresAt: "2026-07-22T11:00:00Z",
        }
      next.invocations[action.idempotencyKey] = {
        id: nextId("inv-demo-", Object.keys(next.invocations).length),
        tenantId: next.principal.tenantId,
        tool: action.tool,
        idempotencyKey: action.idempotencyKey,
        scenario: action.scenario,
        status,
        result,
        approvalId,
      }
      return next
    }
    case "incident.status":
      if (
        !canAccessOperator(state) ||
        next.incidents[action.id]?.tenantId !== state.principal?.tenantId
      )
        return state
      if (next.incidents[action.id])
        Object.assign(next.incidents[action.id], {
          status: action.status,
          note: action.note,
        })
      return next
    case "security-policy.save":
      if (!canAccessOperator(state)) return state
      next.securityPolicies[action.policy.id] = action.policy
      return next
  }
}

export const seedState = createSeedState()
