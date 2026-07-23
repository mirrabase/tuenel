# T04 — tenant URL and isolation

**State:** live.

1. Register account A and note tenant UUID A.
2. Register account B in a private window and note tenant UUID B.
3. Attempt to open `/en/<tenant-uuid-A>/models` while signed in as B.
4. Sign in as A and confirm `/en/<tenant-uuid-A>/models` works.
5. Replace the tenant UUID in the URL with a random valid UUID and refresh.

**Pass:** each account sees only its membership; mismatched or unknown tenant context is rejected or redirected; no tenant credential is accepted for another tenant.
