# rdp-proxy

[![CI](https://github.com/Rip-07/rdp-proxy/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/Rip-07/rdp-proxy/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust 2024](https://img.shields.io/badge/rust-edition%202024-orange.svg)](Cargo.toml)
[![Image](https://img.shields.io/badge/image-ghcr.io%2Frip--07%2Frdp--proxy-2496ED.svg)](https://github.com/Rip-07/rdp-proxy/pkgs/container/rdp-proxy)

Expose one RDP endpoint for a Windows host that is only reachable over a VPN (e.g. WireGuard): a small transparent TCP proxy in Rust with an IP allow-list, connection limit, health check and Prometheus metrics. [English below](#english).

## Быстрый старт

Нужен Linux с Docker. Маршрут до целевого RDP-хоста (например, через WireGuard) уже должен быть на хосте.

```bash
docker run -d --name rdp-proxy --network host \
  -e RDP_PROXY_TARGET_HOST=192.0.2.10 \
  ghcr.io/rip-07/rdp-proxy:0.1.0
curl http://127.0.0.1:8080/health
```

Подключайтесь RDP-клиентом к `<адрес-хоста>:3389`. Health и метрики слушают только `127.0.0.1`, поэтому нужен `--network host`.

Прозрачный TCP-прокси для RDP. Приложение слушает `0.0.0.0:3389` и без какого-либо разбора протокола пересылает байты на целевой хост (например, `192.0.2.10:3389`) через обычную таблицу маршрутизации хоста. WireGuard настраивается отдельно и этим проектом не управляется.

## Сборка

```bash
cargo build --release
```

## Запуск

```bash
cargo run --release
```

По умолчанию читается `config.yaml`, если файл существует. Переменные окружения имеют приоритет над YAML.

`target_host` (`RDP_PROXY_TARGET_HOST`) обязателен и должен быть IP-адресом (DNS-имена пока не поддерживаются): без него прокси завершится с ошибкой конфигурации. Пример переменных — в `.env.example`.

## Конфигурация

Поддерживаемые параметры:

- `listen_host`
- `listen_port`
- `target_host`
- `target_port`
- `connect_timeout_ms`
- `tcp_keepalive`
- `tcp_nodelay`
- `backlog`
- `log_level`

Дополнительные параметры (необязательные):

- `log_format` (`text` или `json`)
- `connection_limit`
- `allowed_ips`

### YAML

```yaml
listen_host: 0.0.0.0
listen_port: 3389
target_host: 192.0.2.10
target_port: 3389
connect_timeout_ms: 10000
tcp_keepalive: true
tcp_nodelay: true
backlog: 1024
log_level: info
log_format: text
connection_limit: 1000
allowed_ips:
  - 198.51.100.0/24
  - 203.0.113.5
```

### Переменные окружения

Имена:

- `RDP_PROXY_LISTEN_HOST`
- `RDP_PROXY_LISTEN_PORT`
- `RDP_PROXY_TARGET_HOST`
- `RDP_PROXY_TARGET_PORT`
- `RDP_PROXY_CONNECT_TIMEOUT_MS`
- `RDP_PROXY_TCP_KEEPALIVE`
- `RDP_PROXY_TCP_NODELAY`
- `RDP_PROXY_BACKLOG`
- `RDP_PROXY_LOG_LEVEL`
- `RDP_PROXY_LOG_FORMAT`
- `RDP_PROXY_CONNECTION_LIMIT`
- `RDP_PROXY_ALLOWED_IPS`

`RDP_PROXY_ALLOWED_IPS` принимает список через запятую, например:

```bash
export RDP_PROXY_ALLOWED_IPS="198.51.100.0/24,203.0.113.5"
```

## Логирование

Поддерживаются:

- текстовый формат
- JSON

Пример:

```bash
export RDP_PROXY_LOG_FORMAT=json
export RDP_PROXY_LOG_LEVEL=info
```

В логах есть:

- новое соединение
- IP клиента
- IP цели
- длительность соединения
- байты в обе стороны
- причина отключения

## Health и метрики

- Health: `http://127.0.0.1:8080/health`
- Prometheus: `http://127.0.0.1:9090/metrics`

Метрики:

- `active_connections`
- `total_connections`
- `bytes_in`
- `bytes_out`
- `failed_connections`

## Docker

Сборка:

```bash
docker build -t rdp-proxy .
```

Запуск:

```bash
docker run --rm --network host -e RDP_PROXY_TARGET_HOST=192.0.2.10 rdp-proxy
```

Compose:

```bash
cp .env.example .env   # укажите RDP_PROXY_TARGET_HOST
docker compose up -d --build
```

Контейнер запускается от непривилегированного пользователя. Встроен `HEALTHCHECK`.

## CI

GitHub Actions (`.github/workflows/ci.yml`) запускается на `push` в `main` и на `pull_request`:

- `cargo fmt --all --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all-targets --all-features`
- `docker build`

## Unraid / homelab

Используйте `docker-compose.yml` или создайте контейнер вручную с `network_mode: host`.

Для Unraid важны три вещи:

1. Host networking должен быть включён.
2. Порт `3389` не должен быть занят другим сервисом.
3. На хосте должна быть уже поднята маршрутизация до целевого хоста (например, `192.0.2.10`) через WireGuard.

## Пример WireGuard

Сеть WireGuard настраивается вне приложения. Пример маршрута на хосте:

```bash
ip route add 192.0.2.0/24 dev wg0
```

После этого прокси будет ходить к `192.0.2.10:3389` через обычный TCP connect.

## Производительность

Реализация использует Tokio TCP sockets и `tokio::io::copy_bidirectional()`. Это подходит для тысяч одновременных сессий при достаточных системных лимитах:

- `ulimit -n`
- размеры backlog
- параметры ядра TCP

## Лицензия

MIT — см. [LICENSE](LICENSE).

## English

**rdp-proxy** is a small transparent TCP proxy for RDP written in Rust (Tokio). It listens on `0.0.0.0:3389` and forwards raw bytes to a target host without parsing the protocol, using the host's normal routing table (for example, a WireGuard tunnel configured separately). Useful in a homelab to expose a single RDP endpoint that lives behind a VPN.

`/health` and `/metrics` listen on `127.0.0.1` only, so run the container with host networking.

Features: YAML + environment configuration, optional IP/CIDR allow-list and connection limit, text/JSON logs with per-connection duration and byte counters, `/health` endpoint (port 8080), Prometheus `/metrics` (port 9090), graceful shutdown, non-root Docker image with `HEALTHCHECK`.

### Configuration

Settings are read from `config.yaml` (if present) and overridden by `RDP_PROXY_*` environment variables. `target_host` is required. See `.env.example` for the full list:

| Variable | Default |
|---|---|
| `RDP_PROXY_TARGET_HOST` | — (required, IP address; DNS names are not supported yet) |
| `RDP_PROXY_TARGET_PORT` | `3389` |
| `RDP_PROXY_LISTEN_HOST` / `RDP_PROXY_LISTEN_PORT` | `0.0.0.0` / `3389` |
| `RDP_PROXY_CONNECT_TIMEOUT_MS` | `10000` |
| `RDP_PROXY_TCP_KEEPALIVE` / `RDP_PROXY_TCP_NODELAY` | `true` / `true` |
| `RDP_PROXY_BACKLOG` | `1024` |
| `RDP_PROXY_LOG_LEVEL` / `RDP_PROXY_LOG_FORMAT` | `info` / `text` |
| `RDP_PROXY_CONNECTION_LIMIT` | unlimited |
| `RDP_PROXY_ALLOWED_IPS` | allow all (comma-separated IPs/CIDRs) |

### Run

```bash
# prebuilt image (Linux, host networking)
docker run -d --network host -e RDP_PROXY_TARGET_HOST=192.0.2.10 ghcr.io/rip-07/rdp-proxy:0.1.0

# from source
RDP_PROXY_TARGET_HOST=192.0.2.10 cargo run --release

# Docker (host networking so the proxy can use the host routes)
docker build -t rdp-proxy .
docker run --rm --network host -e RDP_PROXY_TARGET_HOST=192.0.2.10 rdp-proxy

# Docker Compose
cp .env.example .env   # set RDP_PROXY_TARGET_HOST
docker compose up -d --build
```

### License

MIT — see [LICENSE](LICENSE).
