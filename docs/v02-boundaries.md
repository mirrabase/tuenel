# v0.2 boundary audit

The v0.2 inference path remains provider-neutral inside `gateway-core`: HTTP DTO conversion is in `gateway-server`, authentication normalizes JWTs and Virtual Keys into `Principal`, policy and quota checks precede provider execution, adapters own provider DTOs, and PostgreSQL owns durable usage. Redis is limited to request counters and concurrency reservations. Billing delivery consumes an idempotent PostgreSQL outbox and never blocks inference.

Version 0.3 reuses those boundaries. MCP protocol DTOs terminate in transport adapters and become canonical gateway MCP types. MCP and inference share `Principal`, tenant/project ownership, policy hierarchy, security inspection, audit identity, and request IDs. No agent runtime, autonomous tool selection, tunnel, private networking, dashboard, or CLI is introduced.

The audit found no defect requiring a rewrite. v0.3 therefore extends the existing services and store boundaries rather than replacing them.
