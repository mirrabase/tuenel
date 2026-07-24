# Self-hosting Tuenel

Docker Compose is the supported self-hosting path. Production runs PostgreSQL,
Redis, `gateway`, and `gateway-web`; mock providers and demo data are only in
`compose.dev.yaml`.

For a public domain, automatic HTTPS, private backend networks, and a hardened
Docker socket boundary, use
[`compose.production.yaml`](../compose.production.yaml) with the
[Traefik production guide](production-traefik.md).

## Production quick start

Requirement: Docker Engine with Compose v2.

```sh
./install.sh
```

The installer creates a mode-`0600` `.env`, generates the database password,
credential encryption key, and web session secret, then pulls and starts the
four production services. It never creates demo data. Never commit `.env`.

Open `http://localhost:3000/en/register`, create an account and tenant, then
create a provider and model route in the web console. Tuenel starts safely
without an upstream; inference returns unavailable until that setup is
complete.

For an environment-managed default provider, set `UPSTREAM_BASE_URL`,
`UPSTREAM_MODEL`, and `UPSTREAM_API_KEY` together. OIDC JWT authentication is
optional; enable it by setting `OIDC_ISSUER`, `OIDC_AUDIENCE`, and
`OIDC_JWKS_URL` together. Local browser sessions and virtual keys work without
OIDC. Pin `TUENEL_VERSION=vX.Y.Z` for stable deployments; `latest` follows
`main`.

Before the first release, the maintainers must make both
`tuenel-gateway` GHCR packages public.

To build the checked-out source instead:

```sh
docker compose up -d --build
```

Gateway listens on `8080` and the web console on `3000` by default. Put both
behind a TLS-terminating reverse proxy, expose only the required public
hostname(s), and keep PostgreSQL and Redis private. If using separate
hostnames, route the browser to `gateway-web` and API clients to `gateway`.

## Startup, health, and first tenant

`PostgresStore::connect()` runs the embedded SQLx migrations before the gateway
starts. A connection or migration error terminates startup; a successful run
logs that PostgreSQL connected and migrations were applied. Do not run a
second migration system.

```sh
docker compose ps
curl --fail http://localhost:8080/health
curl --fail http://localhost:8080/ready
curl --fail http://localhost:3000/en/login
```

Production Compose never inserts a demo tenant or provider.

Startup is fatal when a required infrastructure or secret variable is
missing, an optional OIDC/upstream group is only partly configured, the
credential master key is not base64 encoding exactly 32 bytes, or
`WEB_SESSION_SECRET` is shorter than 32 characters. Known development examples
produce a warning that names the variable but never prints its value.

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

## Development and smoke tests

The overlay is the only opt-in path for mock providers and demo tenant seed:

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
