# Production deployment with Traefik

Use `compose.production.yaml` for a single-host, internet-facing deployment.
It runs Tuenel behind Traefik with automatic Let's Encrypt certificates. The
regular `compose.yaml` remains the local/private-network installation path.

## Prerequisites

- A Linux host with Docker Engine and Compose v2.
- A static public IPv4 address. If publishing an AAAA record, the host must
  also be reachable over IPv6.
- DNS A/AAAA records for the web and API hostnames pointing to this host.
- Inbound TCP ports `80` and `443` open. Port `80` must remain reachable for
  Let's Encrypt HTTP-01 issuance and renewal.
- Both Tuenel GHCR packages must be public or the Docker host must already be
  authenticated to GHCR.

Do not publish PostgreSQL, Redis, gateway port `8080`, web port `3000`, the
Traefik dashboard, or the Docker socket proxy.

## Configure

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

Replace both example domains, the ACME email, and `TUENEL_VERSION`. Pin a
published release such as `v1.0.0`; do not deploy `latest`.

OIDC and the environment-managed upstream are optional. When enabled, set
every value in the relevant group. Provider credentials may instead be added
from the console after startup.

## Validate and start

```sh
docker compose --env-file .env.production -f compose.production.yaml config --quiet
docker compose --env-file .env.production -f compose.production.yaml pull
docker compose --env-file .env.production -f compose.production.yaml up -d
docker compose --env-file .env.production -f compose.production.yaml ps
```

Traefik redirects HTTP to HTTPS and obtains separate certificates for the web
and API hostnames. Certificate state is stored in the `traefik-acme` named
volume and renewals are automatic.

Verify DNS, redirects, certificates, and health:

```sh
curl --fail --head "http://tuenel.example.com"
curl --fail "https://api.tuenel.example.com/ready"
curl --fail "https://tuenel.example.com/en/login"
```

Open the web hostname and create the first account. Registration is public at
the web hostname; for an internal-only deployment, restrict the hostname with
your firewall, VPN, or identity-aware edge proxy.

## Exposure and security model

Only Traefik binds host ports. PostgreSQL and Redis are attached only to an
internal Docker network. The API hostname exposes inference/MCP, health, and
OpenAPI paths; administration and browser authentication remain reachable
only through the web service's internal gateway proxy.

Traefik container discovery uses a dedicated Docker socket proxy. It grants
only read access to the container, event, info, network, ping, and version API
sections; write requests are denied. The socket proxy is isolated on its own
internal network.

The Traefik dashboard and API are disabled. Access logs are disabled by
default so credentials or invitation data in URLs cannot be retained by the
edge. Application logs remain available with:

```sh
docker compose --env-file .env.production -f compose.production.yaml logs -f gateway gateway-web traefik
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
docker compose --env-file .env.production -f compose.production.yaml pull
docker compose --env-file .env.production -f compose.production.yaml up -d
```

Rollback by restoring the previous image tag. If a release documents an
incompatible database migration, restore the matching PostgreSQL backup too.

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
