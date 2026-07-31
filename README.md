# Tuenel Gateway

Tuenel is a self-hosted, identity-aware AI gateway for OpenAI-compatible
inference and authorized MCP access.

The Apache-2.0 Community edition is fully usable without an account, license,
or connection to a Tuenel-operated service. Optional commercial extensions
integrate through the public
[extension contract](docs/commercial-extension-contract.md) and do not move
existing Community behavior behind a paywall.

## Quick start

```sh
./install.sh
```

The installer offers direct HTTP, bundled Traefik, and existing Traefik modes.
Direct mode needs no domain or public IP and publishes the web console on
`http://localhost:4050` and the API on `http://localhost:4060`. The installer
prints a one-time setup link for creating the instance administrator and first
organization; it never stores the plaintext setup token or account password.
Then add a provider and model route in the console. OIDC is optional. Use a
published container version such as `TUENEL_VERSION=0.4.2` for stable
deployments. See the
[self-hosting guide](docs/self-hosting.md) for TLS, backups, upgrades, OIDC,
development, and PaaS deployment.

For an internet-facing domain with automatic Let's Encrypt TLS, use the
[production Traefik deployment](docs/production-traefik.md).

For a gateway running directly on the host, start only PostgreSQL and Redis:

```sh
./infra/dev/dev.sh up
```

See the [local development guide](infra/dev/README.md). Mock providers remain
isolated as internal integration-test fixtures and are not part of the normal
development stack.

The gateway reduces AI security risk; it cannot guarantee detection of every
prompt injection, jailbreak, secret leak, or malicious MCP response.
