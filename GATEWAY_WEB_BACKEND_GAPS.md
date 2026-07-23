# Gateway Web v0.3 — Backend Coverage

The authenticated console uses the same-origin `/api/gateway/*` BFF. Its pages
read and mutate PostgreSQL-backed gateway resources; no browser mock store is
used.

Covered surfaces include inference and embeddings, tenants, projects, members,
virtual keys, providers, routing, pricing, policies, quotas, usage, audit,
billing delivery, MCP registry/tools/policies/approvals, and security
policies/patterns/incidents/findings/events.

Native `/mcp` remains machine-only. The console uses the typed `/v1/mcp/*` and
`/admin/mcp/*` APIs instead of exposing raw protocol sessions.

PostgreSQL is durable truth. Redis is limited to counters, reservations, and
caches. Provider credentials and virtual-key plaintext are write-only and are
never logged or persisted by the browser.
