# syntax=docker/dockerfile:1

FROM rust:1-bookworm AS builder
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --release

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --create-home --home-dir /home/rdp-proxy --shell /usr/sbin/nologin rdp-proxy

WORKDIR /home/rdp-proxy

COPY --from=builder /app/target/release/rdp-proxy /usr/local/bin/rdp-proxy

USER rdp-proxy

EXPOSE 3389 8080 9090

HEALTHCHECK --interval=10s --timeout=3s --start-period=10s --retries=3 \
  CMD curl -fsS http://127.0.0.1:8080/health || exit 1

ENTRYPOINT ["rdp-proxy"]
