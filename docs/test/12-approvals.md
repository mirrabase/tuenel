# T13 — approval inbox

**State:** mock fixture.

1. Open **Operator → Approvals**.
2. Open a pending request and inspect tenant/tool/reason/expiry details.
3. Approve it with a reason, then repeat with reject.
4. Try approving/rejecting the same request twice.
5. Let or simulate expiry and refresh/poll the request.
6. Verify the explorer reflects pending, approved, rejected, and expired outcomes.

**Pass:** decisions are idempotent, reasons are sanitized, expired requests cannot be approved, and status transitions are clear.
