# Tuenel Gateway

Tuenel Gateway is a Rust-first AI gateway with OpenAI-compatible HTTP APIs.

This repository currently contains the compile-ready workspace boundaries. Domain logic and external adapters will be added one vertical slice at a time.

## Workspace

- `crates/`: reusable library crates for gateway domain and application boundaries
- `apps/gatewayd`: gateway server binary
- `apps/gateway-cli`: administrative `gateway` binary
- `apps/gateway-migrate`: database migration binary

## Development

```sh
cargo check --workspace --all-targets
cargo test --workspace
```
