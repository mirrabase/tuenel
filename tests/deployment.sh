#!/bin/sh
set -eu

env_file=$(mktemp)
empty_env=$(mktemp)
trap 'rm -f "$env_file" "$empty_env"' EXIT

cat >"$env_file" <<'EOF'
POSTGRES_PASSWORD=test
DATABASE_URL=postgres://gateway:test@postgres:5432/gateway
GATEWAY_CREDENTIALS_MASTER_KEY=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=
WEB_SESSION_SECRET=01234567890123456789012345678901
AUTH_BOOTSTRAP_TOKEN_HASH=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
TUENEL_DEPLOYMENT_MODE=standalone
AUTH_REGISTRATION_MODE=invite_only
AUTH_INVITATION_DELIVERY=manual
TRAEFIK_EXISTING=false
TRAEFIK_NETWORK=tuenel_edge
GATEWAY_PORT=4060
GATEWAY_WEB_PORT=4050
EOF

services=$(docker compose --env-file "$env_file" config --services)
[ "$(printf '%s\n' "$services" | wc -l)" -eq 4 ]
for service in postgres redis gateway gateway-web; do
  printf '%s\n' "$services" | grep -qx "$service"
done
if printf '%s\n' "$services" | grep -Eq 'mock|bootstrap|smoke'; then
  exit 1
fi

direct=$(docker compose --env-file "$env_file" config)
printf '%s\n' "$direct" | grep -q 'published: "4050"'
printf '%s\n' "$direct" | grep -q 'published: "4060"'
printf '%s\n' "$direct" | grep -q 'GATEWAY_PUBLIC_URL: http://localhost:4060/v1'
printf '%s\n' "$direct" | grep -q 'AUTH_REGISTRATION_MODE: invite_only'
printf '%s\n' "$direct" | grep -q 'AUTH_INVITATION_DELIVERY: manual'
printf '%s\n' "$direct" | grep -q 'TUENEL_DEPLOYMENT_MODE: standalone'
printf '%s\n' "$direct" | grep -q 'AUTH_BOOTSTRAP_TOKEN_HASH: aaaaaaaaaa'
printf '%s\n' "$direct" | grep -q 'WEB_SESSION_COOKIE_SECURE: "false"'

host_development=$(docker compose --env-file infra/dev/.env.example -f infra/dev/compose.yaml config)
host_development_services=$(docker compose --env-file infra/dev/.env.example -f infra/dev/compose.yaml config --services)
[ "$(printf '%s\n' "$host_development_services" | wc -l)" -eq 2 ]
for service in postgres redis; do
  printf '%s\n' "$host_development_services" | grep -qx "$service"
done
if printf '%s\n' "$host_development" | grep -Eq 'mock|gateway-web|^[[:space:]]+gateway:'; then
  exit 1
fi
[ "$(printf '%s\n' "$host_development" | grep -c 'host_ip: 127.0.0.1')" -eq 2 ]
printf '%s\n' "$host_development" | grep -q 'published: "5432"'
printf '%s\n' "$host_development" | grep -q 'published: "6379"'
printf '%s\n' "$host_development" | grep -q 'source: postgres-data'
printf '%s\n' "$host_development" | grep -q 'pg_isready -U gateway -d gateway'
printf '%s\n' "$host_development" | grep -q 'image: postgres:16-alpine'
printf '%s\n' "$host_development" | grep -q 'image: redis:7-alpine'

development=$(docker compose --env-file .env.dev.example -f compose.yaml -f compose.dev.yaml --profile test config)
for service in mock-provider mock-anthropic mock-gemini mock-mcp-safe mock-mcp-destructive mock-mcp-malicious mock-billing-webhook bootstrap smoke v03-smoke; do
  printf '%s\n' "$development" | grep -q "^  $service:"
done
printf '%s\n' "$development" | grep -q 'profiles:'
printf '%s\n' "$development" | grep -q -- '- test'

if docker compose --env-file "$empty_env" config >/dev/null 2>&1; then
  exit 1
fi

production_services=$(
  TUENEL_WEB_DOMAIN=tuenel.example.com \
  TUENEL_API_DOMAIN=api.tuenel.example.com \
  ACME_EMAIL=admin@example.com \
  TUENEL_VERSION=v1.0.0 \
  docker compose --env-file "$env_file" -f compose.production.yaml --profile bundled-traefik config --services
)
[ "$(printf '%s\n' "$production_services" | wc -l)" -eq 6 ]
for service in docker-socket-proxy traefik postgres redis gateway gateway-web; do
  printf '%s\n' "$production_services" | grep -qx "$service"
done

production=$(
  TUENEL_WEB_DOMAIN=tuenel.example.com \
  TUENEL_API_DOMAIN=api.tuenel.example.com \
  ACME_EMAIL=admin@example.com \
  TUENEL_VERSION=v1.0.0 \
  docker compose --env-file "$env_file" -f compose.production.yaml --profile bundled-traefik config
)
[ "$(printf '%s\n' "$production" | grep -c 'published:')" -eq 2 ]
printf '%s\n' "$production" | grep -q 'published: "80"'
printf '%s\n' "$production" | grep -q 'published: "443"'
[ "$(printf '%s\n' "$production" | grep -c 'internal: true')" -eq 2 ]
[ "$(printf '%s\n' "$production" | grep -c 'source: /var/run/docker.sock')" -eq 1 ]
printf '%s\n' "$production" | grep -q 'endpoint=tcp://docker-socket-proxy:2375'
printf '%s\n' "$production" | grep -q 'exposedbydefault=false'
printf '%s\n' "$production" | grep -q 'certresolver: letsencrypt'
printf '%s\n' "$production" | grep -q 'PathPrefix(`/v1`)'
printf '%s\n' "$production" | grep -q 'Host(`api.tuenel.example.com`) || Host(`tuenel.example.com`)'
printf '%s\n' "$production" | grep -q 'tuenel-api.priority: "100"'
printf '%s\n' "$production" | grep -q 'GATEWAY_PUBLIC_URL: https://api.tuenel.example.com/v1'
printf '%s\n' "$production" | grep -q 'NEXT_PUBLIC_APP_URL: https://tuenel.example.com'
printf '%s\n' "$production" | grep -q 'WEB_SESSION_COOKIE_SECURE: "true"'
if printf '%s\n' "$production" | grep -Eq 'PathPrefix\\(`/admin`\\)|mock|bootstrap|smoke'; then
  exit 1
fi
if docker compose --env-file "$empty_env" -f compose.production.yaml config >/dev/null 2>&1; then
  exit 1
fi

existing_services=$(
  TUENEL_WEB_DOMAIN=tuenel.example.com \
  TUENEL_API_DOMAIN=api.tuenel.example.com \
  TUENEL_VERSION=v1.0.0 \
  TRAEFIK_EXISTING=true \
  TRAEFIK_NETWORK=existing_edge \
  docker compose --env-file "$env_file" -f compose.production.yaml config --services
)
if printf '%s\n' "$existing_services" | grep -Eq 'traefik|docker-socket-proxy'; then
  exit 1
fi

warning=$(WEB_SESSION_SECRET=development-only-web-session-secret apps/gateway-web/docker-entrypoint.sh true 2>&1)
printf '%s\n' "$warning" | grep -q 'WEB_SESSION_SECRET uses the known development example'
if printf '%s\n' "$warning" | grep -q 'development-only-web-session-secret'; then
  exit 1
fi
if WEB_SESSION_SECRET=short apps/gateway-web/docker-entrypoint.sh true >/dev/null 2>&1; then
  exit 1
fi

sh -n install.sh infra/dev/dev.sh infra/deploy/deploy.sh
./install.sh --help >/dev/null
./infra/dev/dev.sh --help >/dev/null

migration_versions=$(for migration in migrations/*.sql; do
  basename "$migration" | sed 's/_.*//'
done)
[ "$(printf '%s\n' "$migration_versions" | sort | uniq -d | wc -l)" -eq 0 ]

# Published migrations are immutable and auth bootstrap owns version 0009.
[ "$(sha256sum migrations/0006_gateway_control_plane.sql | cut -d ' ' -f 1)" = \
  "9f7acb225fe0cd33bca535726fb3c83a86c7bfab83b210c284d39fb848a76653" ]
[ -f migrations/0009_auth_bootstrap.sql ]
[ ! -f migrations/0009_gateway_control_plane.sql ]
[ ! -f migrations/0006_email_verification.sql ]
grep -q 'installation_id UUID NOT NULL' migrations/0009_auth_bootstrap.sql
grep -q 'CREATE TABLE pending_registrations' migrations/0009_auth_bootstrap.sql

# Public CI is fork-safe and production credentials are absent.
grep -q 'pull_request:' .github/workflows/ci.yml
if grep -R -Eq 'VM_SSH_KEY|VM_HOST|Deploy release to VM' .github/workflows; then
  exit 1
fi
grep -q 'value=edge' .github/workflows/release.yml
grep -q 'type=ref,event=tag' .github/workflows/release.yml
grep -q 'cosign sign' .github/workflows/release.yml
grep -q 'sbom: true' .github/workflows/release.yml
if grep -R -Eq 'value=latest|TUENEL_VERSION:-latest' .github/workflows compose.yaml; then
  exit 1
fi
if grep -R -Eq 'TUENEL_VERSION=v[0-9]|TUENEL_VERSION=vX' README.md docs infra; then
  echo "deployment documentation must use container versions without a v prefix" >&2
  exit 1
fi
