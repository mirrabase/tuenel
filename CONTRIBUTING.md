# Contributing

Issues and pull requests are welcome.

1. Discuss large behavior or schema changes in an issue first.
2. Keep changes focused and add a test for behavior changes.
3. Do not include credentials, tokens, production data, or generated `.env`
   files.
4. Run the repository checks before opening a pull request:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
pnpm --dir apps/gateway-web lint
pnpm --dir apps/gateway-web typecheck
pnpm --dir apps/gateway-web test
pnpm --dir apps/gateway-web build
```

By contributing, you agree that your contribution is licensed under the
repository's [Apache-2.0 license](LICENSE).

