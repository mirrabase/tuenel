# T06 — playground inference

**State:** live.

1. Open **Playground** as an owner/admin.
2. Submit a normal prompt and confirm a response, model name, and usage/error state render.
3. Submit an empty prompt and an overlong prompt.
4. Switch the available request mode between chat, responses, and embeddings if shown.
5. Repeat as a viewer.
6. Refresh and verify the page remains usable without exposing a bearer token.

**Pass:** successful requests use the live gateway; validation/errors are visible and sanitized; viewer inference is denied; no secret is stored in browser-readable storage.
