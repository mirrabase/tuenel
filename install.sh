#!/bin/sh
set -eu

if ! docker compose version >/dev/null 2>&1; then
  echo "Docker Compose v2 is required." >&2
  exit 1
fi

if [ ! -f .env ]; then
  umask 077
  if command -v openssl >/dev/null 2>&1; then
    postgres_password=$(openssl rand -hex 24)
    master_key=$(openssl rand -base64 32 | tr -d '\n')
    session_secret=$(openssl rand -hex 32)
  else
    random_values=$(docker run --rm postgres:16-alpine sh -c \
      'head -c 24 /dev/urandom | od -An -tx1 | tr -d " \n"; echo; head -c 32 /dev/urandom | base64 | tr -d "\n"; echo; head -c 32 /dev/urandom | od -An -tx1 | tr -d " \n"')
    postgres_password=$(printf '%s\n' "$random_values" | sed -n '1p')
    master_key=$(printf '%s\n' "$random_values" | sed -n '2p')
    session_secret=$(printf '%s\n' "$random_values" | sed -n '3p')
  fi
  cat >.env <<EOF
TUENEL_VERSION=latest
POSTGRES_DB=gateway
POSTGRES_USER=gateway
POSTGRES_PASSWORD=$postgres_password
DATABASE_URL=postgres://gateway:$postgres_password@postgres:5432/gateway
GATEWAY_CREDENTIALS_MASTER_KEY=$master_key
WEB_SESSION_SECRET=$session_secret
GATEWAY_PORT=8080
GATEWAY_WEB_PORT=3000
DEFAULT_VIRTUAL_KEY_DAILY_TOKENS=100000
RUST_LOG=info
MCP_ENABLED=true
EOF
  echo "Created .env with generated secrets."
else
  echo "Using existing .env."
fi

docker compose pull
docker compose up -d
echo "Tuenel is starting at http://localhost:${GATEWAY_WEB_PORT:-3000}/en/register"
