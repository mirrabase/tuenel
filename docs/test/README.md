# UI test suite

Manual end-to-end tests for `gateway-web`. Run them against `http://localhost:3000` after:

```powershell
docker compose --env-file .env.dev.example -f compose.yaml -f compose.dev.yaml up -d --build
```

Create a fresh email per run. Tests marked **live** call the gateway through the BFF and must survive a page refresh. Tests marked **mock** verify the current browser fixture/reducer behavior; they are not persistence tests.

Run in order: setup, auth, tenant/RBAC, live workspace, then mock operator surfaces.

| File | Flow | State |
|---|---|---|
| [00-setup](./00-setup.md) | service and browser setup | live |
| [01-auth](./01-auth.md) | register, login, invalid credentials | live |
| [02-session-logout](./02-session-logout.md) | session persistence and logout | live |
| [03-tenant-isolation](./03-tenant-isolation.md) | tenant URL and session binding | live |
| [04-members-rbac](./04-members-rbac.md) | invite and role access | live |
| [05-playground](./05-playground.md) | chat, responses, embeddings | live |
| [06-models](./06-models.md) | model discovery | live |
| [07-virtual-keys](./07-virtual-keys.md) | issue and revoke key | live |
| [08-usage-docs](./08-usage-docs.md) | usage and API docs | fixture/docs |
| [09-mcp-registry](./09-mcp-registry.md) | MCP CRUD and discovery | mock |
| [10-mcp-policies](./10-mcp-policies.md) | MCP policy lifecycle | mock |
| [11-mcp-explorer](./11-mcp-explorer.md) | tool call, block, redact, warn | mock |
| [12-approvals](./12-approvals.md) | pending/approve/reject/expire | mock |
| [13-security](./13-security.md) | policy and incident operations | mock |
| [14-platform-ops](./14-platform-ops.md) | platform operation pages | mock |
| [15-navigation-i18n](./15-navigation-i18n.md) | routes, locale, responsive UI | live |
| [16-negative-access](./16-negative-access.md) | invalid routes and permissions | live/mock |

Record pass/fail, browser, commit, and timestamp beside each test when executing.
