# T01 — setup and health

**Expected:** local environment is usable.

1. From the repository root run `docker compose up -d --build`.
2. Confirm `gateway` and `gateway-web` are healthy with `docker compose ps`.
3. Open `http://localhost:3000/en/login`; confirm the two-column auth layout loads.
4. Open `http://localhost:8080/ready`; confirm the gateway returns a ready response.
5. Use desktop width, mobile width, and a private browser window for later auth tests.

**Pass:** no console error prevents rendering and both health checks succeed.
