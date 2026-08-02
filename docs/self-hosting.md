# Self-hosting Tuenel

Docker Compose is the supported self-hosting path. Production runs PostgreSQL,
Redis, `gateway`, and `gateway-web`; mock providers and demo data are only in
`compose.dev.yaml`.

For a public domain, automatic HTTPS, private backend networks, and a hardened
Docker socket boundary, use
[`compose.production.yaml`](../compose.production.yaml) with the
[Traefik production guide](production-traefik.md).

## Installation modes

Requirement: Docker Engine with Compose v2.

```sh
./install.sh
```

The installer offers:

- **Direct HTTP:** no domain, public IP, or Traefik is required. The web
  console listens on every host interface at port `4050` and the API at
  `4060`. This mode has no TLS; restrict it with a firewall and do not expose
  it directly to the internet. Direct mode permits the session cookie over
  HTTP so remote LAN/VPN clients can use the console; use this mode only on a
  trusted network.
- **Bundled Traefik:** runs the hardened production Compose stack with
  automatic Let's Encrypt certificates.
- **Existing Traefik:** joins an existing external Traefik Docker network.

The installer creates a mode-`0600` `.env` or `.env.production`, generates the
database password, credential encryption key, web session secret, and a
one-time bootstrap token, then pulls and starts the selected services. Only
the token's SHA-256 digest is stored. The interactive terminal prints a setup
link once; the browser removes its URL fragment before submitting the token.
The installer never accepts or persists the account password, never overwrites
an existing env file, and never creates demo data. Never commit either env
file.

For direct mode, open the printed `http://localhost:4050/en/setup#token=...`
link and create the instance administrator and first organization. The claim
is serialized in PostgreSQL, so only one concurrent request can become the
instance administrator. Then create a provider and model route in the web
console. Tuenel starts safely without an upstream; inference returns
unavailable until that setup is complete.

Self-hosted installations default to
`TUENEL_DEPLOYMENT_MODE=standalone`,
`AUTH_REGISTRATION_MODE=invite_only`, and
`AUTH_INVITATION_DELIVERY=manual`. Owners receive a one-time invitation link
for each new member. Set invitation delivery to `email` only after configuring
Resend. `closed` disables both public and invitation-based account creation.

Shared managed deployments use `TUENEL_DEPLOYMENT_MODE=managed` and explicitly
choose `public`, `invite_only`, or `closed`. Public registration and email
invitation delivery require `RESEND_API_KEY`, `RESEND_FROM`, and
`NEXT_PUBLIC_APP_URL`; incomplete configuration is fatal at startup.

For an environment-managed default provider, set `UPSTREAM_BASE_URL`,
`UPSTREAM_MODEL`, and `UPSTREAM_API_KEY` together. OIDC JWT authentication is
optional; enable it by setting `OIDC_ISSUER`, `OIDC_AUDIENCE`, and
`OIDC_JWKS_URL` together. Local browser sessions and virtual keys work without
OIDC. Pin `TUENEL_VERSION=X.Y.Z` for stable deployments; `edge` follows
`main`. No `latest` production tag is published.

Before the first release, the maintainers must make both
`tuenel-gateway` GHCR packages public.

To build the checked-out source with the direct Compose stack instead:

```sh
docker compose up -d --build
```

Fresh direct installations publish gateway on `4060` and the web console on
`4050`. Put public deployments behind a TLS-terminating reverse proxy, expose
only the required hostname(s), and keep PostgreSQL and Redis private. If using
separate hostnames, route the browser to `gateway-web` and API clients to
`gateway`.

## Startup, health, and first tenant

`PostgresStore::connect()` runs the embedded SQLx migrations before the gateway
starts. A connection or migration error terminates startup; a successful run
logs that PostgreSQL connected and migrations were applied. Do not run a
second migration system.

```sh
docker compose ps
curl --fail http://localhost:4060/health
curl --fail http://localhost:4060/ready
curl --fail http://localhost:4050/en/login
```

Production Compose never inserts a demo tenant or provider.

Startup is fatal when a required infrastructure or secret variable is
missing, an optional OIDC/upstream group is only partly configured, the
credential master key is not base64 encoding exactly 32 bytes, or
`WEB_SESSION_SECRET` is shorter than 32 characters. An empty database also
requires `AUTH_BOOTSTRAP_TOKEN_HASH`. Known development examples produce a
warning that names the variable but never prints its value.

## Backup, upgrade, and rollback

PostgreSQL is durable truth. Back it up on a schedule; Redis contains only
counters, reservations, and caches.

```sh
docker compose exec -T postgres pg_dump -U gateway -d gateway -Fc > tuenel.dump
```

Before an upgrade, take a backup and read the release notes. Pin the new tag
and let the gateway apply migrations:

```sh
# Edit TUENEL_VERSION in .env
docker compose pull
docker compose up -d
```

For an application rollback, restore the previous `TUENEL_VERSION` and repeat
the commands. Database migrations are forward-applied; if a release documents
an incompatible migration, restore the matching PostgreSQL backup as part of
the rollback.

## MCP and private-network security

Production Compose forces `MCP_ALLOW_PRIVATE_HTTP_ENDPOINTS=false`. This blocks
HTTP MCP endpoints that resolve to loopback, link-local, multicast,
unspecified, or private addresses. Keep it disabled for internet-facing
deployments. The development overlay enables it only so local mock MCP servers
work. Review allowed stdio commands and MCP policies before enabling MCP for
tenants.

## Host development

Start the host-development PostgreSQL and Redis services:

```sh
./infra/dev/dev.sh up
```

Both services bind only to `127.0.0.1`. PostgreSQL data persists in a named
volume, while Redis remains ephemeral. See
[`infra/dev/README.md`](../infra/dev/README.md) for lifecycle, connection, and
host gateway commands.

## Internal integration fixtures

Mock providers and demo tenant data are test-only fixtures. They are not part
of the normal developer or deployment path. Maintainers can run the isolated
compatibility stack with:

```sh
docker compose --env-file .env.dev.example -f compose.yaml -f compose.dev.yaml up -d --build
docker compose --env-file .env.dev.example -f compose.yaml -f compose.dev.yaml --profile test run --rm smoke
docker compose --env-file .env.dev.example -f compose.yaml -f compose.dev.yaml --profile test run --rm v03-smoke
```

The example development secrets are public and must never be reused in
production.

## PaaS

No vendor manifest is maintained; translate the production Compose services
and variables in the provider dashboard.

- **Railway:** map each Compose service to a separate Railway service, replace
  the database containers with managed PostgreSQL and Redis, and use reference
  variables plus private networking. Railway does not execute Compose
  directly. See the [Railway Compose guide](https://docs.railway.com/guides/docker-compose).
- **Coolify:** import `compose.yaml` with the Docker Compose build pack as the
  source of truth, add the required environment variables, and attach domains
  to gateway port `8080` and web port `3000`. See the
  [Coolify Compose documentation](https://coolify.io/docs/applications/build-packs/docker-compose).
- **Render:** create two Docker image services from the matching GHCR tags,
  plus managed PostgreSQL and Key Value, and connect them over private
  networking. See Render's [Docker](https://render.com/docs/docker) and
  [service types](https://render.com/docs/service-types) documentation.

## Troubleshooting

- **Compose reports a missing variable:** fill every empty field in `.env`;
  production interpolation intentionally has no secret fallback.
- **Gateway restarts before becoming ready:** inspect
  `docker compose logs gateway`; invalid configuration, PostgreSQL migration,
  Redis, and configured JWKS initialization failures are fatal.
- **Web exits immediately:** ensure `WEB_SESSION_SECRET` is at least 32
  characters.
- **OIDC returns 401/403:** verify all three OIDC values are set, issuer and audience match the token exactly,
  the JWKS URL is reachable from the gateway, and the tenant exists.
- **Upstream calls fail:** include the provider's expected `/v1` base path and
  verify its API key and model name.
- **MCP private endpoint is rejected:** use a public HTTPS endpoint or a
  development-only overlay; do not weaken the production SSRF boundary without
  accepting the network exposure.
