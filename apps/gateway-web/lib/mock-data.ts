export type CapabilityStatus = "Available" | "Preview" | "Planned"

export const usageSeries = [
  { day: "Mon", input: 31800, output: 12400 },
  { day: "Tue", input: 40200, output: 15100 },
  { day: "Wed", input: 37600, output: 13900 },
  { day: "Thu", input: 53100, output: 20400 },
  { day: "Fri", input: 48700, output: 18900 },
  { day: "Sat", input: 29100, output: 9700 },
  { day: "Sun", input: 44600, output: 16700 },
]

export const usageEvents = [
  {
    id: "req_8f1a",
    model: "gateway-default",
    provider: "openai-compatible",
    tokens: 2841,
    cost: "$0.0184",
    latency: "1.8s",
    status: "Succeeded",
    time: "2 min ago",
    estimated: false,
  },
  {
    id: "req_3c72",
    model: "fast-chat",
    provider: "local-vllm",
    tokens: 1190,
    cost: "$0.0021",
    latency: "640ms",
    status: "Succeeded",
    time: "8 min ago",
    estimated: false,
  },
  {
    id: "req_54de",
    model: "gateway-default",
    provider: "openai-compatible",
    tokens: 4096,
    cost: "$0.0266",
    latency: "3.2s",
    status: "Interrupted",
    time: "14 min ago",
    estimated: true,
  },
  {
    id: "req_a903",
    model: "reasoning",
    provider: "openai-compatible",
    tokens: 8210,
    cost: "$0.0912",
    latency: "7.9s",
    status: "Provider failed",
    time: "31 min ago",
    estimated: false,
  },
  {
    id: "req_b114",
    model: "fast-chat",
    provider: "local-vllm",
    tokens: 902,
    cost: "$0.0017",
    latency: "510ms",
    status: "Succeeded",
    time: "44 min ago",
    estimated: false,
  },
]

export const models = [
  {
    alias: "gateway-default",
    upstream: "gpt-4.1-mini",
    provider: "openai-compatible",
    input: "$0.40",
    output: "$1.60",
    context: "128k",
    status: "Available" as CapabilityStatus,
  },
  {
    alias: "fast-chat",
    upstream: "meta-llama/Llama-3.3-70B",
    provider: "local-vllm",
    input: "$0.12",
    output: "$0.30",
    context: "32k",
    status: "Preview" as CapabilityStatus,
  },
  {
    alias: "reasoning",
    upstream: "o4-mini",
    provider: "openai-compatible",
    input: "$1.10",
    output: "$4.40",
    context: "200k",
    status: "Planned" as CapabilityStatus,
  },
]

export const initialKeys = [
  {
    id: "key_01JZX8",
    name: "Production API",
    prefix: "vk_live_7f3a••••",
    scope: "chat",
    quota: "100k/day",
    lastUsed: "2 min ago",
    status: "Active",
  },
  {
    id: "key_01JXV2",
    name: "Evaluation runner",
    prefix: "vk_live_91bd••••",
    scope: "chat",
    quota: "25k/day",
    lastUsed: "Yesterday",
    status: "Active",
  },
  {
    id: "key_01JWP9",
    name: "Old staging",
    prefix: "vk_live_2cc1••••",
    scope: "chat",
    quota: "10k/day",
    lastUsed: "Jun 18",
    status: "Revoked",
  },
]

export const tenants = [
  {
    id: "acme-production",
    quota: "1.0M",
    used: "68%",
    requests: "24.8k",
    cost: "$318.20",
    status: "Healthy",
  },
  {
    id: "northstar-labs",
    quota: "500k",
    used: "91%",
    requests: "18.1k",
    cost: "$241.08",
    status: "Quota risk",
  },
  {
    id: "internal-tools",
    quota: "250k",
    used: "34%",
    requests: "6.7k",
    cost: "$42.17",
    status: "Healthy",
  },
  {
    id: "sandbox",
    quota: "100k",
    used: "12%",
    requests: "1.2k",
    cost: "$8.31",
    status: "Healthy",
  },
]

export const providers = [
  {
    name: "OpenAI compatible",
    id: "openai-compatible",
    baseUrl: "https://api.openai.com/v1",
    models: 2,
    latency: "1.8s",
    errorRate: "0.14%",
    secret: "Configured",
    status: "Healthy",
  },
  {
    name: "Local vLLM",
    id: "local-vllm",
    baseUrl: "https://inference.internal/v1",
    models: 1,
    latency: "640ms",
    errorRate: "0.08%",
    secret: "Not required",
    status: "Healthy",
  },
  {
    name: "Regional fallback",
    id: "regional-fallback",
    baseUrl: "https://llm.ap-southeast/v1",
    models: 0,
    latency: "—",
    errorRate: "—",
    secret: "Configured",
    status: "Planned",
  },
]

export const modelRoutes = [
  {
    alias: "gateway-default",
    provider: "openai-compatible",
    upstream: "gpt-4.1-mini",
    priority: "Primary",
    status: "Active",
  },
  {
    alias: "fast-chat",
    provider: "local-vllm",
    upstream: "meta-llama/Llama-3.3-70B",
    priority: "Primary",
    status: "Preview",
  },
  {
    alias: "reasoning",
    provider: "openai-compatible",
    upstream: "o4-mini",
    priority: "Primary",
    status: "Planned",
  },
]

export const pricingRules = [
  {
    model: "gateway-default",
    input: "$0.40",
    output: "$1.60",
    effective: "Jul 1, 2026",
    version: "v4",
    status: "Active",
  },
  {
    model: "fast-chat",
    input: "$0.12",
    output: "$0.30",
    effective: "Jul 1, 2026",
    version: "v2",
    status: "Active",
  },
  {
    model: "reasoning",
    input: "$1.10",
    output: "$4.40",
    effective: "Aug 1, 2026",
    version: "v1",
    status: "Draft",
  },
]

export const policies = [
  {
    name: "Production model access",
    target: "acme-production",
    rule: "Allow gateway-default, fast-chat",
    priority: 10,
    status: "Preview",
  },
  {
    name: "Evaluation token ceiling",
    target: "scope:evaluation",
    rule: "Max 2,048 output tokens",
    priority: 20,
    status: "Preview",
  },
  {
    name: "Block unpriced models",
    target: "All tenants",
    rule: "Deny models without active pricing",
    priority: 30,
    status: "Planned",
  },
]

export const reservations = [
  {
    id: "rsv_9e21",
    owner: "acme-production",
    kind: "Tenant",
    tokens: "6,144",
    expires: "in 3m",
    status: "Active",
  },
  {
    id: "rsv_1a4c",
    owner: "key_01JZX8",
    kind: "Virtual key",
    tokens: "2,560",
    expires: "in 4m",
    status: "Active",
  },
  {
    id: "rsv_63bf",
    owner: "northstar-labs",
    kind: "Tenant",
    tokens: "4,096",
    expires: "Reconciled",
    status: "Released",
  },
]

export const integrations = [
  {
    name: "MCP control plane",
    description:
      "Expose gateway discovery and administration to trusted agents.",
    delivery: "Not connected",
    status: "Planned",
  },
  {
    name: "Usage event sink",
    description: "Deliver idempotent usage events without blocking inference.",
    delivery: "99.98% delivered",
    status: "Preview",
  },
  {
    name: "Billing adapter",
    description: "Forward normalized usage to an external billing system.",
    delivery: "No target",
    status: "Planned",
  },
]

export const healthChecks = [
  {
    name: "Gateway API",
    detail: "HTTP and SSE accepting traffic",
    value: "Operational",
  },
  {
    name: "PostgreSQL",
    detail: "Durable tenants, quotas, and usage",
    value: "Connected · 12ms",
  },
  {
    name: "OIDC / JWKS",
    detail: "RS256 issuer keys cached",
    value: "Healthy · refreshed 4m ago",
  },
  {
    name: "Reservation reconciler",
    detail: "Expired quota reservations",
    value: "0 pending",
  },
]

export const requiredRoutes = [
  "/",
  "/playground",
  "/models",
  "/keys",
  "/usage",
  "/docs",
  "/operator",
  "/operator/tenants",
  "/operator/providers",
  "/operator/routing",
  "/operator/pricing",
  "/operator/policies",
  "/operator/ledger",
  "/operator/system",
  "/operator/integrations",
] as const
