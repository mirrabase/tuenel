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
EOF

services=$(docker compose --env-file "$env_file" config --services)
[ "$(printf '%s\n' "$services" | wc -l)" -eq 4 ]
for service in postgres redis gateway gateway-web; do
  printf '%s\n' "$services" | grep -qx "$service"
done
if printf '%s\n' "$services" | grep -Eq 'mock|bootstrap|smoke'; then
  exit 1
fi

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
  docker compose --env-file "$env_file" -f compose.production.yaml config --services
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
  docker compose --env-file "$env_file" -f compose.production.yaml config
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
if printf '%s\n' "$production" | grep -Eq 'PathPrefix\\(`/admin`\\)|mock|bootstrap|smoke'; then
  exit 1
fi
if docker compose --env-file "$empty_env" -f compose.production.yaml config >/dev/null 2>&1; then
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

sh -n install.sh
