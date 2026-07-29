# Local development infrastructure

This Compose project runs only the infrastructure required by a
host-running gateway: PostgreSQL 16 and Redis 7. Both ports are bound to
loopback. PostgreSQL data is kept in a named Docker volume; Redis contains
only counters, reservations, and caches and is intentionally ephemeral.

## Quick start

From the repository root:

```sh
./infra/dev/dev.sh up
```

The script creates `infra/dev/.env` from safe development defaults only when
the file does not exist. It never overwrites an existing file.

Run the gateway directly on the host:

```sh
set -a
. ./infra/dev/.env
set +a
cargo run -p gatewayd
```

`PostgresStore::connect()` applies the repository's embedded SQLx migrations
before the gateway starts. Redis must also be available because gateway
startup verifies its connection.

## Docker Compose commands

Run these commands from `infra/dev`:

```sh
# Start PostgreSQL and Redis
docker compose up -d

# Check status
docker compose ps

# View PostgreSQL logs
docker compose logs -f postgres

# View Redis logs
docker compose logs -f redis

# Stop without deleting data
docker compose down

# Full development database reset
docker compose down -v
```

Use `./infra/dev/dev.sh check` from the repository root to validate the
rendered Compose configuration, PostgreSQL readiness and queries, and Redis
connectivity.
