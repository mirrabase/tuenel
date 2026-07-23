# T07 — models

**State:** live.

1. Open **Models**.
2. Confirm the live model list loads, including the configured gateway alias.
3. Refresh and temporarily stop the gateway, then reload to verify the error state.
4. Restart Compose and confirm recovery.

**Pass:** the page distinguishes loading, success, and failure; model data comes from `/v1/models` through the BFF.
