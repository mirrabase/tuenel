# Agent Guidelines

- Keep business logic out of HTTP and MCP handlers.
- Keep `gateway-core` independent of concrete providers, stores, authentication, and transports.
- Use canonical gateway types internally; provider-specific DTOs stay in adapters.
- Never log or persist plaintext credentials, bearer tokens, or virtual keys.
- Treat PostgreSQL as durable truth; Redis is only for counters, reservations, and caches.
- Make usage and billing events idempotent, and never block inference on billing delivery.
- Add only dependencies and abstractions required by the current vertical slice.
- Add tests with every behavior change and run format, lint, and test checks before completion.
