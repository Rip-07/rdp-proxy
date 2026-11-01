# Changelog

## 0.1.0 — 2026-11-01

First public release.

- Transparent TCP proxy for RDP built on Tokio (`copy_bidirectional`), no protocol parsing.
- Configuration from `config.yaml` with `RDP_PROXY_*` environment overrides; `target_host` is required.
- Optional client IP/CIDR allow-list and connection limit.
- Text or JSON logs with client/target address, duration, bytes in both directions and disconnect reason.
- `/health` on `127.0.0.1:8080` and Prometheus `/metrics` on `127.0.0.1:9090`.
- Graceful shutdown.
- Docker image running as a non-root user, with `HEALTHCHECK`; CI with fmt, clippy, tests and image build.
