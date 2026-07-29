# Production deployment with Traefik

Use `compose.production.yaml` for a single-host, internet-facing deployment.
The installer activates its bundled Traefik profile for automatic Let's
Encrypt certificates. Set `TRAEFIK_EXISTING=true` and omit that profile to use
an existing Traefik instead. The regular `compose.yaml` remains the direct
HTTP path.

## Prerequisites

- A Linux host with Docker Engine and Compose v2.
- A static public IPv4 address. If publishing an AAAA record, the host must
  also be reachable over IPv6.
- DNS A/AAAA records for the web and API hostnames pointing to this host.
- Inbound TCP ports `80` and `443` open. Port `80` must remain reachable for
  Let's Encrypt HTTP-01 issuance and renewal.
- Both Tuenel GHCR packages must be public or the Docker host must already be
  authenticated to GHCR.

When using an existing Traefik, it must already be attached to the configured
Docker network and have Docker provider access. The bundled Traefik and socket
proxy are disabled in that mode.

Do not publish PostgreSQL, Redis, gateway port `8080`, web port `3000`, the
Traefik dashboard, or the Docker socket proxy.

## Configure

The recommended path is the first-run wizard:

```sh
./install.sh
```

Choose bundled or existing Traefik. To configure the environment manually:

```sh
cp .env.production.example .env.production
chmod 600 .env.production
openssl rand -hex 24
openssl rand -base64 32
openssl rand -hex 32
```

Use the first value as `POSTGRES_PASSWORD`, the second as
`GATEWAY_CREDENTIALS_MASTER_KEY`, and the third as `WEB_SESSION_SECRET`. Set
`DATABASE_URL` to
`postgres://gateway:<URL-encoded-password>@postgres:5432/gateway`.

Replace both example domains and `TUENEL_VERSION`. Bundled Traefik also
requires the ACME email. Pin a published release such as `v0.4.0`; do not
deploy the moving `edge` tag.

OIDC and the environment-managed upstream are optional. When enabled, set
every value in the relevant group. Provider credentials may instead be added
from the console after startup.

The wizard separately asks for the identity topology. Use `standalone` for a
self-hosted or dedicated installation. For a shared managed control plane use
`managed`, then choose `public`, `invite_only`, or `closed`. Public signup and
email invitations require a verified Resend sender. The first account is
always created through the protected one-time setup link and is the only local
account automatically granted instance-administrator privileges.

## Validate and start

```sh
docker compose --env-file .env.production -f compose.production.yaml --profile bundled-traefik config --quiet
docker compose --env-file .env.production -f compose.production.yaml --profile bundled-traefik pull
docker compose --env-file .env.production -f compose.production.yaml --profile bundled-traefik up -d
docker compose --env-file .env.production -f compose.production.yaml --profile bundled-traefik ps
```

Traefik redirects HTTP to HTTPS and obtains separate certificates for the web
and API hostnames. Certificate state is stored in the `traefik-acme` named
volume and renewals are automatic.

For an existing Traefik, omit `--profile bundled-traefik` from every command.

Verify DNS, redirects, certificates, and health:

```sh
curl --fail --head "http://tuenel.example.com"
curl --fail "https://api.tuenel.example.com/ready"
curl --fail "https://tuenel.example.com/ready"
curl --fail "https://tuenel.example.com/en/login"
```

`https://api.tuenel.example.com/v1` is the canonical OpenAI-compatible base
URL shown by the console. The same approved public paths are also routed on
the web hostname for compatibility, so existing clients using
`https://tuenel.example.com/v1` continue to work. Set `GATEWAY_PUBLIC_URL` only
when the canonical browser-facing base URL differs from the API hostname.

Open the one-time setup link printed by the installer. Normal registration is
exposed only when `AUTH_REGISTRATION_MODE=public`; customer accounts become
owners of their own organization but never instance administrators. For an
internal-only deployment, also restrict the hostname with your firewall, VPN,
or identity-aware edge proxy.

## Exposure and security model

Only Traefik binds host ports. PostgreSQL and Redis are attached only to an
internal Docker network. The API hostname and the compatibility routes on the
web hostname expose inference/MCP, health, and OpenAPI paths; administration
and browser authentication remain reachable only through the web service's
internal gateway proxy.

Traefik container discovery uses a dedicated Docker socket proxy. It grants
only read access to the container, event, info, network, ping, and version API
sections; write requests are denied. The socket proxy is isolated on its own
internal network.

The Traefik dashboard and API are disabled. Access logs are disabled by
default. Bootstrap, verification, and invitation secrets are carried in URL
fragments, which are never sent to the edge, and are removed from browser
history immediately. Application logs remain available with:

```sh
docker compose --env-file .env.production -f compose.production.yaml --profile bundled-traefik logs -f gateway gateway-web traefik
```

Security headers include one-year HSTS without `includeSubDomains`, frame
denial, MIME sniffing protection, and a strict-origin referrer policy. Do not
enable HSTS for all subdomains until every subdomain is permanently HTTPS.

If another CDN or load balancer sits in front of Traefik, configure explicit
trusted forwarded-header or PROXY-protocol CIDRs before using client IPs for
security decisions. Never enable Traefik's insecure forwarded-header mode.

## Backup, upgrade, and rollback

PostgreSQL is durable truth:

```sh
docker compose --env-file .env.production -f compose.production.yaml exec -T postgres pg_dump -U gateway -d gateway -Fc > tuenel.dump
```

Back up `.env.production` and the `traefik-acme` volume securely as well.
They contain encryption material, database credentials, and certificate
private keys.

For upgrades, take a database backup, change the pinned `TUENEL_VERSION`, then:

```sh
docker compose --env-file .env.production -f compose.production.yaml --profile bundled-traefik pull
docker compose --env-file .env.production -f compose.production.yaml --profile bundled-traefik up -d
```

Rollback by restoring the previous image tag. If a release documents an
incompatible database migration, restore the matching PostgreSQL backup too.

## Deployment automation

The public repository does not deploy production infrastructure and holds no
VM or application secrets. `.github/workflows/ci.yml` runs secret-free checks
for pull requests and pushes. `.github/workflows/release.yml` publishes signed,
attested images: immutable version tags come from Git tags and `edge` tracks
`main`.

Keep production deployment credentials in the deployment environment, not the
public repository. A deployment system needs:

- `VM_HOST`: VM hostname or IP.
- `VM_USER`: SSH user with permission to run Docker Compose.
- `VM_SSH_KEY`: private Ed25519 key; install its public key in the user's
  `~/.ssh/authorized_keys` on the VM.
- `VM_KNOWN_HOSTS`: pinned host key data.
- `POSTGRES_PASSWORD`
- `DATABASE_URL`
- `GATEWAY_CREDENTIALS_MASTER_KEY`
- `WEB_SESSION_SECRET`
- `AUTH_BOOTSTRAP_TOKEN_HASH` (required only until a fresh database is claimed)
- `RESEND_API_KEY` (required for public signup or email invitations)
- `UPSTREAM_API_KEY` (only if using an environment-managed upstream).

Add these non-secret GitHub Actions variables to the same `production`
environment:

```text
COMPOSE_PROJECT_NAME=tuenel
TRAEFIK_EXISTING=false
TRAEFIK_NETWORK=tuenel_edge
TUENEL_WEB_DOMAIN=tuenel.example.com
TUENEL_API_DOMAIN=api.tuenel.example.com
ACME_EMAIL=admin@example.com
POSTGRES_DB=gateway
POSTGRES_USER=gateway
TUENEL_DEPLOYMENT_MODE=standalone
AUTH_REGISTRATION_MODE=invite_only
AUTH_INVITATION_DELIVERY=manual
RESEND_FROM=
OIDC_ISSUER=
OIDC_AUDIENCE=
OIDC_JWKS_URL=
OIDC_ADMIN_ROLE=gateway_admin
UPSTREAM_BASE_URL=
UPSTREAM_MODEL=
GATEWAY_MODEL_ALIAS=gateway-default
DEFAULT_VIRTUAL_KEY_DAILY_TOKENS=100000
RUST_LOG=info
MCP_ENABLED=true
TRAEFIK_LOG_LEVEL=INFO
```

The workflow pins `TUENEL_VERSION` automatically to `main`. The
environment file is written with mode `0600` and is never printed in logs.

For an existing Traefik, set:

```text
TRAEFIK_EXISTING=true
TRAEFIK_NETWORK=<existing-docker-network>
TRAEFIK_SECURE_ENTRYPOINT=websecure
TRAEFIK_CERTRESOLVER=letsencrypt
```

The workflow then omits the bundled Traefik and socket proxy. The existing
Traefik must route the labels on `gateway` and `gateway-web`, and must be
connected to `<existing-docker-network>`.

The existing Traefik must define a `websecure` entrypoint on `:443` and a
`letsencrypt` ACME certificate resolver. Tuenel only adds router labels; it
cannot configure an external Traefik's ACME storage or entrypoints.

If the GHCR packages are private, authenticate Docker on the VM once with a
read-only package token; do not put that token in the repository or workflow:

```sh
echo '<read-only-ghcr-token>' | docker login ghcr.io -u '<github-user>' --password-stdin
```

After pushing to `main`, CI passes first and then the deploy job copies the
production Compose file, pins the `main` image, pulls
the images, and recreates the services. The workflow does not transfer or
print application secrets; they remain in `/opt/tuenel/.env.production`.

## Troubleshooting

- **No certificate:** verify both DNS records resolve to the host and inbound
  ports `80/443` are reachable. Remove a broken AAAA record if IPv6 is not
  routed.
- **Default certificate or TLS error:** inspect Traefik logs and confirm the
  ACME email/domain values are not examples.
- **404 on the API hostname:** only approved public API paths are exposed.
  Use the web console for browser authentication and administration.
- **502/504:** inspect gateway/web health and logs. The edge read timeout is
  `180s`, longer than the default gateway request timeout.
- **Docker provider unavailable:** inspect `docker-socket-proxy`; on hosts with
  SELinux, allow the socket bind mount according to the host policy rather
  than exposing the Docker API publicly.

This Compose file is for one Traefik replica on one Docker host. The ACME file
must not be shared concurrently by multiple Traefik instances; use an
orchestrator-specific certificate solution when high availability is needed.
