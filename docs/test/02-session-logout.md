# T03 — session persistence and logout

**State:** live.

1. Sign in and refresh the tenant console.
2. Close and reopen the browser tab at the tenant URL.
3. Confirm the session remains active without credentials appearing in the URL, page text, localStorage, or sessionStorage.
4. Click **Sign out**.
5. Visit the previous tenant URL and `/en/login` again.

**Pass:** refresh preserves the session; logout clears access and redirects unauthenticated navigation to login.
