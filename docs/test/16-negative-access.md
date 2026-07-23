# T17 — negative access and trust boundaries

1. Open `/en/<random-uuid>` without a session.
2. Call the console through a browser with a different `Origin` and submit a mutation.
3. Try changing the tenant UUID in an invite/API URL while keeping the original session.
4. Use an expired, malformed, reused, or foreign invite token.
5. Attempt admin mutation and inference as viewer.
6. Inspect network responses and browser storage for bearer/session secrets.

**Pass:** unauthenticated access redirects, cross-origin mutations are rejected, tenant mismatch is forbidden, bad invites fail generically, RBAC is enforced, and credentials exist only in hardened HttpOnly cookies/server forwarding.
