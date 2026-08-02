#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "$script_dir/../.." && pwd)
direct_env="$repo_root/.env"
production_env="$repo_root/.env.production"

usage() {
  cat <<'EOF'
Usage: ./install.sh [mode]

Modes:
  direct               Expose web on :4050 and API on :4060 without Traefik
  bundled-traefik      Run the production stack with bundled Traefik and ACME
  existing-traefik     Attach the production stack to an existing Traefik network
  help                 Show this help

With no mode, an interactive terminal shows a setup wizard. First-time
non-interactive direct installs must provide a high-entropy
TUENEL_BOOTSTRAP_TOKEN; only its hash is written to the environment file.
EOF
}

require_tools() {
  if ! docker compose version >/dev/null 2>&1; then
    echo "Docker Compose v2 is required." >&2
    exit 1
  fi
}

generate_secrets() {
  if command -v openssl >/dev/null 2>&1; then
    postgres_password=$(openssl rand -hex 24)
    master_key=$(openssl rand -base64 32 | tr -d '\n')
    session_secret=$(openssl rand -hex 32)
    bootstrap_token="tb_$(openssl rand -hex 32)"
  else
    random_values=$(docker run --rm postgres:16-alpine sh -c \
      'head -c 24 /dev/urandom | od -An -tx1 | tr -d " \n"; echo; head -c 32 /dev/urandom | base64 | tr -d "\n"; echo; head -c 32 /dev/urandom | od -An -tx1 | tr -d " \n"')
    postgres_password=$(printf '%s\n' "$random_values" | sed -n '1p')
    master_key=$(printf '%s\n' "$random_values" | sed -n '2p')
    session_secret=$(printf '%s\n' "$random_values" | sed -n '3p')
    bootstrap_token="tb_$(docker run --rm postgres:16-alpine sh -c 'head -c 32 /dev/urandom | od -An -tx1 | tr -d \" \\n\"')"
  fi
  if command -v sha256sum >/dev/null 2>&1; then
    bootstrap_hash=$(printf '%s' "$bootstrap_token" | sha256sum | sed 's/[[:space:]].*$//')
  else
    bootstrap_hash=$(printf '%s' "$bootstrap_token" | openssl dgst -sha256 | sed 's/^.* //')
  fi
}

prompt() {
  prompt_label=$1
  prompt_default=${2:-}
  if [ -n "$prompt_default" ]; then
    printf "%s [%s]: " "$prompt_label" "$prompt_default" >&2
  else
    printf "%s: " "$prompt_label" >&2
  fi
  read -r prompt_value
  printf '%s\n' "${prompt_value:-$prompt_default}"
}

prompt_required() {
  required_label=$1
  required_default=${2:-}
  while :; do
    required_value=$(prompt "$required_label" "$required_default")
    if [ -n "$required_value" ]; then
      printf '%s\n' "$required_value"
      return
    fi
    echo "A value is required." >&2
  done
}

prompt_secret_required() {
  secret_label=$1
  while :; do
    printf "%s: " "$secret_label" >&2
    secret_tty=$(stty -g)
    stty -echo
    if ! IFS= read -r secret_value; then
      stty "$secret_tty"
      return 1
    fi
    stty "$secret_tty"
    printf '\n' >&2
    if [ -n "$secret_value" ]; then
      printf '%s\n' "$secret_value"
      return
    fi
    echo "A value is required." >&2
  done
}

env_value() {
  env_name=$1
  env_path=$2
  sed -n "s/^${env_name}=//p" "$env_path" | tail -n 1
}

create_direct_env() {
  bootstrap_token=
  show_bootstrap_token=false
  if [ -f "$direct_env" ]; then
    echo "Using existing $direct_env without modifying it."
    return
  fi

  if [ ! -t 0 ] && [ -z "${TUENEL_BOOTSTRAP_TOKEN:-}" ]; then
    echo "First-time non-interactive installs must provide TUENEL_BOOTSTRAP_TOKEN securely." >&2
    exit 1
  fi
  generate_secrets
  if [ -n "${TUENEL_BOOTSTRAP_TOKEN:-}" ]; then
    bootstrap_token=$TUENEL_BOOTSTRAP_TOKEN
    case "$bootstrap_token" in tb_????????????????????????????????*) ;; *)
      echo "TUENEL_BOOTSTRAP_TOKEN must start with tb_ and contain at least 32 secret characters." >&2
      exit 1
    esac
    if command -v sha256sum >/dev/null 2>&1; then
      bootstrap_hash=$(printf '%s' "$bootstrap_token" | sha256sum | sed 's/[[:space:]].*$//')
    else
      bootstrap_hash=$(printf '%s' "$bootstrap_token" | openssl dgst -sha256 | sed 's/^.* //')
    fi
  else
    show_bootstrap_token=true
  fi
  umask 077
  cat >"$direct_env" <<EOF
TUENEL_VERSION=latest
POSTGRES_DB=gateway
POSTGRES_USER=gateway
POSTGRES_PASSWORD=$postgres_password
DATABASE_URL=postgres://gateway:$postgres_password@postgres:5432/gateway
GATEWAY_CREDENTIALS_MASTER_KEY=$master_key
WEB_SESSION_SECRET=$session_secret
WEB_SESSION_COOKIE_SECURE=false
TUENEL_DEPLOYMENT_MODE=standalone
AUTH_REGISTRATION_MODE=invite_only
AUTH_INVITATION_DELIVERY=manual
AUTH_BOOTSTRAP_TOKEN_HASH=$bootstrap_hash
GATEWAY_PORT=4060
GATEWAY_WEB_PORT=4050
DEFAULT_VIRTUAL_KEY_DAILY_TOKENS=100000
RUST_LOG=info
MCP_ENABLED=true
EOF
  chmod 600 "$direct_env"
  echo "Created $direct_env with generated secrets."
}

create_production_env() {
  production_mode=$1
  bootstrap_token=
  show_bootstrap_token=false
  if [ -f "$production_env" ]; then
    echo "Using existing $production_env without modifying it."
    return
  fi
  if [ ! -t 0 ]; then
    echo "$production_env is missing; run the installer interactively for first-time production setup." >&2
    exit 1
  fi

  echo "Creating a production environment. Existing files are never overwritten."
  tuenel_version=$(prompt_required "Published Tuenel container version (for example 1.0.0)")
  if [ "$tuenel_version" = "latest" ]; then
    echo "Production requires a pinned release tag, not latest." >&2
    exit 1
  fi
  web_domain=$(prompt_required "Web domain")
  api_domain=$(prompt_required "API domain")
  compose_project=$(prompt_required "Compose project name" "tuenel")
  secure_entrypoint=$(prompt_required "Traefik secure entrypoint" "websecure")
  certresolver=$(prompt_required "Traefik certificate resolver" "letsencrypt")
  deployment_mode=$(prompt_required "Deployment identity mode (standalone or managed)" "standalone")
  case "$deployment_mode" in
    standalone)
      registration_mode=$(prompt_required "Registration mode (invite_only or closed)" "invite_only")
      invitation_delivery=$(prompt_required "Invitation delivery (manual or email)" "manual")
      ;;
    managed)
      registration_mode=$(prompt_required "Registration mode (public, invite_only, or closed)" "public")
      invitation_delivery=$(prompt_required "Invitation delivery (manual or email)" "email")
      ;;
    *)
      echo "Deployment identity mode must be standalone or managed." >&2
      exit 1
      ;;
  esac
  case "$registration_mode" in public|invite_only|closed) ;; *)
    echo "Invalid registration mode." >&2
    exit 1
  esac
  case "$invitation_delivery" in manual|email) ;; *)
    echo "Invalid invitation delivery." >&2
    exit 1
  esac
  if [ "$registration_mode" = "public" ] || [ "$invitation_delivery" = "email" ]; then
    resend_api_key=$(prompt_secret_required "Resend API key")
    resend_from=$(prompt_required "Verified sender" "Tuenel <noreply@example.com>")
  else
    resend_api_key=
    resend_from=
  fi

  if [ "$production_mode" = "bundled-traefik" ]; then
    traefik_existing=false
    traefik_network=$(prompt_required "Docker edge network" "${compose_project}_edge")
    acme_email=$(prompt_required "Let's Encrypt account email")
  else
    traefik_existing=true
    traefik_network=$(prompt_required "Existing Traefik Docker network")
    acme_email=
  fi

  generate_secrets
  show_bootstrap_token=true
  umask 077
  cat >"$production_env" <<EOF
COMPOSE_PROJECT_NAME=$compose_project
TRAEFIK_EXISTING=$traefik_existing
TRAEFIK_NETWORK=$traefik_network
TRAEFIK_SECURE_ENTRYPOINT=$secure_entrypoint
TRAEFIK_CERTRESOLVER=$certresolver
TUENEL_WEB_DOMAIN=$web_domain
TUENEL_API_DOMAIN=$api_domain
ACME_EMAIL=$acme_email
TUENEL_VERSION=$tuenel_version
POSTGRES_DB=gateway
POSTGRES_USER=gateway
POSTGRES_PASSWORD=$postgres_password
DATABASE_URL=postgres://gateway:$postgres_password@postgres:5432/gateway
GATEWAY_CREDENTIALS_MASTER_KEY=$master_key
WEB_SESSION_SECRET=$session_secret
WEB_SESSION_COOKIE_SECURE=true
TUENEL_DEPLOYMENT_MODE=$deployment_mode
AUTH_REGISTRATION_MODE=$registration_mode
AUTH_INVITATION_DELIVERY=$invitation_delivery
AUTH_BOOTSTRAP_TOKEN_HASH=$bootstrap_hash
RESEND_API_KEY=$resend_api_key
RESEND_FROM=$resend_from
NEXT_PUBLIC_APP_URL=https://$web_domain
OIDC_ISSUER=
OIDC_AUDIENCE=
OIDC_JWKS_URL=
OIDC_ADMIN_ROLE=gateway_admin
UPSTREAM_BASE_URL=
UPSTREAM_API_KEY=
UPSTREAM_MODEL=
GATEWAY_MODEL_ALIAS=gateway-default
DEFAULT_VIRTUAL_KEY_DAILY_TOKENS=100000
RUST_LOG=info
MCP_ENABLED=true
TRAEFIK_LOG_LEVEL=INFO
EOF
  chmod 600 "$production_env"
  echo "Created $production_env with generated secrets."
}

validate_production_mode() {
  requested_mode=$1
  configured_existing=$(env_value TRAEFIK_EXISTING "$production_env")
  case "$requested_mode:$configured_existing" in
    bundled-traefik:false|existing-traefik:true) ;;
    bundled-traefik:*)
      echo "$production_env must set TRAEFIK_EXISTING=false for bundled Traefik." >&2
      exit 1
      ;;
    existing-traefik:*)
      echo "$production_env must set TRAEFIK_EXISTING=true for existing Traefik." >&2
      exit 1
      ;;
  esac

  configured_version=$(env_value TUENEL_VERSION "$production_env")
  if [ -z "$configured_version" ] || [ "$configured_version" = "latest" ]; then
    echo "$production_env must pin TUENEL_VERSION to a published release tag." >&2
    exit 1
  fi

  if [ "$requested_mode" = "bundled-traefik" ]; then
    configured_email=$(env_value ACME_EMAIL "$production_env")
    if [ -z "$configured_email" ]; then
      echo "$production_env must set ACME_EMAIL for bundled Traefik." >&2
      exit 1
    fi
  fi
}

install_direct() {
  create_direct_env
  gateway_port=$(env_value GATEWAY_PORT "$direct_env")
  web_port=$(env_value GATEWAY_WEB_PORT "$direct_env")
  gateway_port=${gateway_port:-8080}
  web_port=${web_port:-3000}
  echo "Direct mode exposes HTTP without TLS on every host interface."
  echo "Web: http://HOST_IP:$web_port  API: http://HOST_IP:$gateway_port"
  docker compose --env-file "$direct_env" -f "$repo_root/compose.yaml" config --quiet
  docker compose --env-file "$direct_env" -f "$repo_root/compose.yaml" pull
  docker compose --env-file "$direct_env" -f "$repo_root/compose.yaml" up -d
  docker compose --env-file "$direct_env" -f "$repo_root/compose.yaml" ps
  if [ "$show_bootstrap_token" = true ] && [ -n "$bootstrap_token" ] && [ -t 1 ]; then
    echo "One-time setup link: http://localhost:$web_port/en/setup#token=$bootstrap_token"
  fi
}

install_production() {
  production_mode=$1
  create_production_env "$production_mode"
  validate_production_mode "$production_mode"

  if [ "$production_mode" = "existing-traefik" ]; then
    configured_network=$(env_value TRAEFIK_NETWORK "$production_env")
    if [ -z "$configured_network" ] || ! docker network inspect "$configured_network" >/dev/null 2>&1; then
      echo "Existing Traefik network '$configured_network' was not found." >&2
      exit 1
    fi
    set -- docker compose --env-file "$production_env" -f "$repo_root/compose.production.yaml"
  else
    set -- docker compose --env-file "$production_env" -f "$repo_root/compose.production.yaml" --profile bundled-traefik
  fi

  "$@" config --quiet
  "$@" pull
  "$@" up -d
  "$@" ps
  if [ "$show_bootstrap_token" = true ] && [ -n "$bootstrap_token" ] && [ -t 1 ]; then
    echo "One-time setup link: https://$(env_value TUENEL_WEB_DOMAIN "$production_env")/en/setup#token=$bootstrap_token"
  fi
}

interactive_mode() {
  cat >&2 <<'EOF'
Tuenel installation
  1) Direct HTTP (no domain, public IP, or Traefik required)
  2) Bundled Traefik with automatic Let's Encrypt TLS
  3) Existing Traefik
  0) Exit
EOF
  while :; do
    printf "Choose a deployment mode: " >&2
    read -r choice
    case "$choice" in
      1) printf '%s\n' direct; return ;;
      2) printf '%s\n' bundled-traefik; return ;;
      3) printf '%s\n' existing-traefik; return ;;
      0) printf '%s\n' exit; return ;;
      *) echo "Invalid choice." >&2 ;;
    esac
  done
}

require_tools
mode=${1:-}
if [ -z "$mode" ]; then
  if [ -t 0 ]; then
    mode=$(interactive_mode)
  else
    mode=direct
  fi
fi

case "$mode" in
  direct) install_direct ;;
  bundled-traefik|existing-traefik) install_production "$mode" ;;
  exit) exit 0 ;;
  help|-h|--help) usage ;;
  *)
    usage >&2
    exit 2
    ;;
esac
