# T10 — MCP registry

**State:** mock fixture.

1. Open **Operator → MCP Registry**.
2. Create a server with safe name, HTTP endpoint, transport, and write-only secret.
3. Edit its name/endpoint, toggle enablement, run health check, refresh discovery, and open details.
4. Add invalid/private/malformed endpoints and duplicate names.
5. Delete the server and reload.

**Pass:** validation, optimistic UI state, health/discovery states, secret redaction, delete confirmation, and empty/error states behave correctly. Do not treat reload persistence as a backend guarantee yet.
