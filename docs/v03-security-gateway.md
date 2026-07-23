# AI security gateway v0.3

## Architecture

Inference follows authentication → normalized principal → model policy → input inspection → quota → provider routing/fallback → optional output inspection → usage, audit, and billing outbox. MCP follows authentication → the same principal → tenant/project server lookup → filtered discovery/tool policy → risk classification → argument inspection → optional approval → quota → transport invocation → result inspection → MCP usage and audit.

The gateway registers, filters, authorizes, and proxies tools. It never chooses a tool or runs an agent loop.

## Registering and discovering MCP servers

Administrators use `POST /admin/mcp/servers`, then `POST /admin/mcp/servers/{server_id}/refresh`. Safe server views and tool responses omit credentials, secret references, command arguments, environment values, and private transport configuration.

For streamable HTTP, provide `transport_type: "streamable_http"` and an already reachable `http` or `https` endpoint. Redirects are disabled and DNS results are checked against loopback, link-local, multicast, unspecified, and private ranges. Hosted deployments should keep `MCP_ALLOW_PRIVATE_HTTP_ENDPOINTS=false`; enabling it transfers network-reachability responsibility to the administrator.

For stdio, provide `transport_type: "stdio"` and a command present in `MCP_ALLOWED_STDIO_COMMANDS`. The child receives only explicitly decrypted environment entries, has stdin/stdout framed as JSON lines, is deadline-bound, kills on drop, drains stderr without logging its contents, and is terminated on shutdown.

`GET /v1/mcp/tools` and native MCP `tools/list` return only tools allowed for the authenticated principal. The gateway-specific call endpoint is `POST /v1/mcp/tools/call`; stateless native MCP clients may use `POST /mcp`. Tool schemas are size/depth/property/string bounded, normalized, hashed, and treated as untrusted.

## MCP authorization, risk, and metering

MCP policies bind at global, tenant, project, principal, or Virtual Key scope. Child policy resolution intersects allowlists, unions denylists, chooses lower limits, and keeps the stricter action. Policies can restrict arguments by JSON Pointer and allowed values.

Risk is `read_only`, `mutating`, `destructive`, `privileged`, or `unknown`. Administrator annotations win, followed by protocol annotations and conservative name/description heuristics. Unknown is not safe by default. Destructive, privileged, and unknown calls require approval unless a stricter policy denies them.

Redis atomically enforces calls per minute/day and per-server/per-tool concurrency; PostgreSQL remains the durable invocation ledger. Request/response byte limits and execution deadlines are enforced. Usage records include tenant, project, principal, server, tool, latency, bytes, risk, approval, and outcome. MCP tools do not receive monetary prices in v0.3.

## Human approval

A sensitive call returns HTTP 202 with `approval_required` and an approval ID. An administrator reads and decides it through `/admin/approvals`. The original principal retries the identical call with `approval_id` and an `Idempotency-Key`. The approval is tenant-, principal-, request-hash-, expiration-, and idempotency-bound. A completed retry returns the stored result; claimed or uncertain executions cannot replay. This is poll-and-resume, not durable agent suspension.

## Inspection and policies

Security inspectors cover LLM input, tool arguments/results, and optionally model output. Deterministic detectors recognize common API keys, bearer/JWT tokens, private-key headers, AWS/GitHub credentials, credentialed database URLs, high-entropy credential-like strings, email, phone, validated payment-card-like values, IP addresses, prompt-injection patterns, jailbreak structure, and tenant custom regexes.

Security actions are ordered `block > require_approval > redact > warn > allow`. A child policy can tighten but cannot weaken a parent. Mandatory inspection fails closed; `fail_open: true` must be explicitly configured in the applicable parent policy. Redaction uses only sanitized offsets and evidence; full detected values, prompts, responses, MCP arguments, and credentials are not persisted by default.

Security policy administration is under `/admin/security/policies`; findings, events, and incidents are under `/admin/security/findings`, `/admin/security/events`, and `/admin/security/incidents`. Incidents retain request references, hashes/redacted evidence, decision metadata, a bounded sanitized summary, status, and timeline—not raw content.

## Configuration

Required v0.3 settings include `REDIS_URL` and a base64-encoded 32-byte `GATEWAY_CREDENTIALS_MASTER_KEY`. Relevant controls are `MCP_ENABLED`, discovery/schema/result limits, tool timeout, private-endpoint permission, stdio allowlist, `SECURITY_ENABLED`, output inspection defaults in stored security policy, `APPROVAL_ENABLED`, and approval expiration. Startup rejects an invalid master key, URL, timeout, size, or empty allowlist entry. See `.env.example` for the complete environment form.

## Migration from v0.2

Back up PostgreSQL, deploy the v0.3 binary, and let embedded migrations apply in order. Migrations only add columns, indexes, triggers, and new tables; v0.2 tenant, key, usage, pricing, provider, routing, billing, and quota rows remain intact. Keep PostgreSQL available until migrations finish, then deploy all gateway replicas before enabling MCP/security policies. Existing OpenAI Chat Completions, Responses, and Embeddings clients require no request changes.

## Threat model and deployment responsibility

The gateway defends its trust boundaries against cross-tenant access, unsafe MCP schemas/results, SSRF, unapproved executable launch, leaked stored credentials, approval replay, excessive payloads/runtime/concurrency, and known deterministic secret/sensitive-data/prompt-injection patterns. PostgreSQL credentials, host isolation, outbound firewalling, TLS termination, OIDC correctness, executable installation, detector policy tuning, retention, and incident response remain operator responsibilities. Hosted operators should deny private MCP destinations; self-hosters who enable them must control routing and DNS.

The gateway reduces AI security risk. It does not guarantee detection of every prompt injection, jailbreak, secret leak, or malicious MCP response. Deterministic rules and heuristics have false positives and false negatives; optional classifiers require independent recursion protection and are not enabled by default.
