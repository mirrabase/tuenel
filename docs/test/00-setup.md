# T01 — setup and health

**Expected:** local environment is usable.

1. Generate a one-time `tb_` setup token in your shell and export only its
   SHA-256 digest as `AUTH_BOOTSTRAP_TOKEN_HASH`; do not write the raw token to
   a file or command history.

   ```sh
   bootstrap_token="tb_$(openssl rand -hex 32)"
   export AUTH_BOOTSTRAP_TOKEN_HASH="$(printf '%s' "$bootstrap_token" | sha256sum | cut -d' ' -f1)"
   ```
2. From the repository root run `docker compose --env-file .env.dev.example -f compose.yaml -f compose.dev.yaml up -d --build`.
3. Open `/en/setup#token=<the token still held in your shell>`, create the
   local instance administrator, and confirm the fragment disappears from the
   address bar.
4. Confirm `gateway` and `gateway-web` are healthy with `docker compose --env-file .env.dev.example -f compose.yaml -f compose.dev.yaml ps`.
5. Open `http://localhost:3000/en/login`; confirm the two-column auth layout loads.
6. Open `http://localhost:8080/ready`; confirm the gateway returns a ready response.
7. Use desktop width, mobile width, and a private browser window for later auth tests.

**Pass:** no console error prevents rendering and both health checks succeed.
