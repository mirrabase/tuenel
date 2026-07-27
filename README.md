# Tuenel Gateway

Tuenel is a self-hosted, identity-aware AI gateway for OpenAI-compatible
inference and authorized MCP access.

## Quick start

```sh
./install.sh
```

Open `http://localhost:3000/en/register`, create the first account, then add a
provider and model route in the console. OIDC is optional. Use a release tag
such as `TUENEL_VERSION=v1.0.0` for stable deployments. See the
[self-hosting guide](docs/self-hosting.md) for TLS, backups, upgrades, OIDC,
development, and PaaS deployment.

For an internet-facing domain with automatic Let's Encrypt TLS, use the
[production Traefik deployment](docs/production-traefik.md).

For local development with mocks and demo data:

```sh
docker compose --env-file .env.dev.example -f compose.yaml -f compose.dev.yaml up -d --build
```

The gateway reduces AI security risk; it cannot guarantee detection of every
prompt injection, jailbreak, secret leak, or malicious MCP response.
