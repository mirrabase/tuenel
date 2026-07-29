#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
compose_file="$script_dir/compose.yaml"
env_file="$script_dir/.env"
example_file="$script_dir/.env.example"

usage() {
  cat <<'EOF'
Usage: ./infra/dev/dev.sh [command]

Commands:
  up                  Start PostgreSQL and Redis and wait until healthy
  ps                  Show service status
  logs [service]      Follow logs for postgres, redis, or both
  down                Stop services without deleting PostgreSQL data
  reset [--yes]       Stop services and delete the development database
  check               Validate configuration and test both connections
  help                Show this help

With no command, an interactive menu is shown.
EOF
}

require_compose() {
  if ! docker compose version >/dev/null 2>&1; then
    echo "Docker Compose v2 is required." >&2
    exit 1
  fi
}

ensure_env() {
  if [ ! -f "$env_file" ]; then
    umask 077
    cp "$example_file" "$env_file"
    chmod 600 "$env_file"
    echo "Created $env_file from safe development defaults."
  fi
}

dc() {
  docker compose --env-file "$env_file" -f "$compose_file" "$@"
}

prepare() {
  require_compose
  ensure_env
}

check_connections() {
  dc exec -T postgres sh -c 'pg_isready -U "$POSTGRES_USER" -d "$POSTGRES_DB"'
  dc exec -T postgres sh -c 'psql -v ON_ERROR_STOP=1 -U "$POSTGRES_USER" -d "$POSTGRES_DB" -c "SELECT 1"'
  dc exec -T redis redis-cli ping
}

run_up() {
  prepare
  dc config --quiet
  dc up -d --wait
  check_connections
  echo "Development infrastructure is ready."
  echo "PostgreSQL: 127.0.0.1:5432"
  echo "Redis:      127.0.0.1:6379"
}

run_ps() {
  prepare
  dc ps
}

run_logs() {
  prepare
  service=${1:-}
  case "$service" in
    "")
      dc logs -f postgres redis
      ;;
    postgres|redis)
      dc logs -f "$service"
      ;;
    *)
      echo "Unknown service: $service (expected postgres or redis)." >&2
      exit 2
      ;;
  esac
}

run_down() {
  prepare
  dc down
}

run_reset() {
  prepare
  if [ "${1:-}" != "--yes" ]; then
    if [ ! -t 0 ]; then
      echo "Reset requires --yes when input is not interactive." >&2
      exit 2
    fi
    printf "Delete the complete development PostgreSQL volume? [y/N] "
    read -r answer
    case "$answer" in
      y|Y|yes|YES) ;;
      *)
        echo "Reset cancelled."
        return
        ;;
    esac
  fi
  dc down -v
  echo "Development database volume deleted."
}

run_check() {
  prepare
  dc config --quiet
  check_connections
  echo "Compose configuration, PostgreSQL, and Redis checks passed."
}

interactive_menu() {
  while :; do
    cat <<'EOF'

Tuenel development infrastructure
  1) Start PostgreSQL and Redis
  2) Show status
  3) Follow PostgreSQL logs
  4) Follow Redis logs
  5) Stop without deleting data
  6) Reset development database
  0) Exit
EOF
    printf "Choose an action: "
    read -r choice
    case "$choice" in
      1) run_up ;;
      2) run_ps ;;
      3) run_logs postgres ;;
      4) run_logs redis ;;
      5) run_down ;;
      6) run_reset ;;
      0) exit 0 ;;
      *) echo "Invalid choice." >&2 ;;
    esac
  done
}

command=${1:-}
case "$command" in
  "") interactive_menu ;;
  up) run_up ;;
  ps) run_ps ;;
  logs) run_logs "${2:-}" ;;
  down) run_down ;;
  reset) run_reset "${2:-}" ;;
  check) run_check ;;
  help|-h|--help) usage ;;
  *)
    usage >&2
    exit 2
    ;;
esac
