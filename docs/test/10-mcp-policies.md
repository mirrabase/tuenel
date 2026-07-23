# T11 — MCP policies

**State:** mock fixture.

1. Open **Operator → MCP Policies**.
2. Create a policy with allow/deny, tenant/tool scope, limits, and a version.
3. Edit it, toggle it, save it, and cancel an edit.
4. Submit missing scope, invalid limit, and conflicting precedence values.
5. Delete the policy and verify the empty state.

**Pass:** field validation, precedence display, dirty-state handling, confirmation, and error/success notifications work without exposing secrets.
