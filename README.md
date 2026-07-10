# rdp-proxy

Прозрачный TCP-прокси для RDP. Приложение слушает `0.0.0.0:3389` и без какого-либо разбора протокола пересылает байты на `192.0.2.10:3389` через обычную таблицу маршрутизации хоста. WireGuard настраивается отдельно и этим проектом не управляется.

## Сборка

```bash
cargo build --release
```

## Запуск

```bash
cargo run --release
```

По умолчанию читается `config.yaml`, если файл существует. Переменные окружения имеют приоритет над YAML.

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

Бонусные параметры:

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
docker run --rm --network host rdp-proxy
```

Compose:

```bash
docker compose up -d --build
```

Контейнер запускается от непривилегированного пользователя. Встроен `HEALTHCHECK`.

## Gitea CI/CD

Workflow-файл лежит в `.gitea/workflows/ci.yml`.

CI запускается на `pull_request` и на `push` в `main` или `master`:

- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo check --all-targets --all-features`
- `cargo test --all-targets --all-features`

CD запускается автоматически на `push` в `main` или `master` и выполняет полную схему:

1. `cargo fmt --check`
2. `cargo clippy --all-targets --all-features -- -D warnings`
3. `cargo check --all-targets --all-features`
4. `cargo test --all-targets --all-features`
5. синхронизация репозитория в `/opt/rdp-proxy`
6. `docker compose up -d --build --remove-orphans`

Валидация идет на обычном runner, а деплой должен идти на self-hosted runner на Unraid-хосте.

Для этого на runner-хосте должны быть доступны:

- Docker
- Docker Compose
- сеть host networking

Registry не нужен.

## Unraid

Используйте `docker-compose.yml` или создайте контейнер вручную с `network_mode: host`.

Для Unraid важны три вещи:

1. Host networking должен быть включён.
2. Порт `3389` не должен быть занят другим сервисом.
3. На хосте должна быть уже поднята маршрутизация до `192.0.2.10` через WireGuard.

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

MIT
