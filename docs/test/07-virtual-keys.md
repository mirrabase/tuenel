# T08 — virtual keys

**State:** live.

1. Open **Virtual Keys** as owner/admin.
2. Create a key with a name, scope, and daily limit.
3. Confirm the secret is displayed only once; copy it and reload the page.
4. Revoke the key and confirm the UI marks it revoked.
5. Try create/revoke as engineer and viewer.
6. Submit invalid name/limit values.

**Pass:** create/revoke calls the gateway, plaintext is not persisted or shown after reload, and role restrictions/errors are clear.
