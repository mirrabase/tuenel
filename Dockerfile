FROM rust:1.85-bookworm AS builder
WORKDIR /src
COPY . .
RUN cargo build --locked --release -p gatewayd

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 10001 gateway
COPY --from=builder /src/target/release/gatewayd /usr/local/bin/gatewayd
USER gateway
EXPOSE 8080
ENTRYPOINT ["gatewayd"]
