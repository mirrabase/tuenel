# Tuenel Gateway

Tuenel Gateway v0.3 is a Rust-first, identity-aware AI security gateway for OpenAI-compatible inference and authorized MCP access. Existing inference clients keep their `base_url` and credentials; the same JWT or Virtual Key principal is reused for model and MCP policy, quotas, usage, security events, and audit records.

## Architecture

The Axum transport owns OpenAI DTOs and error mapping. `gateway-core` owns the inference pipeline, while authentication, routing, quota, metering, provider, and store crates expose narrow interfaces. Provider DTOs never enter the core, and plaintext credentials never enter logs or storage.

```text
HTTP/MCP -> JWT/Virtual Key -> Principal -> hierarchical policy -> security inspection
         -> inference provider or authorized MCP transport -> usage/audit finalization
```

Private tunnels, VPNs, agent runtimes, autonomous tool selection, dashboards, CLIs, and workflow orchestration remain outside v0.3.

## Docker quick start

Start the gateway, PostgreSQL, development tenant bootstrap, and mock OpenAI/JWKS service:

```sh
docker compose up --build
```

Fetch a short-lived development JWT:

```sh
curl http://localhost:4010/token
```

Run the Python OpenAI SDK compatibility workflow for JWT and Virtual Key streaming:

```sh
docker compose --profile test run --rm smoke
```

The mock issuer and bootstrap tenant are development-only. Production JWT tenants must be provisioned explicitly:

```sql
INSERT INTO tenants (id, daily_token_limit) VALUES ('tenant-id', 1000000);
```

Unknown JWT tenants receive HTTP 403.

## JWT mode

Configure one RS256 OIDC issuer with `OIDC_ISSUER`, `OIDC_AUDIENCE`, and `OIDC_JWKS_URL`. Tokens must contain `sub`, `tenant_id`, `exp`, `iss`, and `aud`; `roles` is optional. Issuer, audience, expiration, algorithm, key ID, and signature are validated before a `Principal` is created.

```python
from openai import OpenAI

client = OpenAI(base_url="http://localhost:8080/v1", api_key=user_jwt)
response = client.chat.completions.create(
    model="gateway-default",
    messages=[{"role": "user", "content": "hello"}],
)
print(response.choices[0].message.content)
```

## Virtual Key mode

A JWT principal with the configured `OIDC_ADMIN_ROLE` may issue keys only for its own tenant. Plaintext is returned once; PostgreSQL stores an Argon2id hash and non-secret lookup prefix.

```sh
curl -X POST http://localhost:8080/admin/virtual-keys \
  -H "Authorization: Bearer $JWT" \
  -H "Content-Type: application/json" \
  -d '{"daily_token_limit":100000,"scopes":["chat"]}'

curl -X DELETE http://localhost:8080/admin/virtual-keys/$KEY_ID \
  -H "Authorization: Bearer $JWT"
```

Use the returned `vk_live_...` value as the OpenAI client's `api_key`. Virtual Keys have independent daily quotas; JWT traffic uses the tenant quota. Days reset at UTC midnight.

## Streaming

Set `stream=True` normally. The gateway parses and re-emits SSE incrementally and propagates disconnects by dropping the upstream stream. Provider usage is preferred; when absent, conservative byte-based estimates are recorded and marked as estimated internally.

## Configuration

Copy `.env.example`. Required values are `DATABASE_URL`, OIDC issuer/audience/JWKS URL, `UPSTREAM_BASE_URL`, and `UPSTREAM_MODEL`. `UPSTREAM_API_KEY` is optional for local providers. The public alias defaults to `gateway-default`; pricing rates default to zero.

The upstream base URL may target OpenAI, vLLM, Ollama's OpenAI mode, LocalAI, or another compatible `/v1` endpoint.

## API and OpenAPI

Core routes include:

- `GET /health`, `GET /ready`, `GET /openapi.json`
- `GET /v1/models`
- `POST /v1/chat/completions`, `POST /v1/responses`, `POST /v1/embeddings`
- `GET /v1/mcp/servers`, `GET /v1/mcp/tools`, `POST /v1/mcp/tools/call`, `POST /mcp`
- `/admin/mcp/*`, `/admin/security/*`, and `/admin/approvals/*`
- `POST /admin/virtual-keys`
- `DELETE /admin/virtual-keys/{id}`

The generated runtime specification is available at `/openapi.json`. Detailed MCP setup, risk/approval behavior, security policy configuration, migration, threat model, and deployment responsibilities are documented in [docs/v03-security-gateway.md](docs/v03-security-gateway.md).

Regenerate the committed specification:

```sh
cargo run -p gatewayd -- openapi > docs/openapi.json
```

## Development checks

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Integration tests requiring PostgreSQL use the same migrations as `gatewayd`. Usage rows are append-only and unique by request ID; quota reservations are reconciled transactionally.

The gateway reduces AI security risk. It does not guarantee detection of every prompt injection, jailbreak, secret leak, or malicious MCP response.
