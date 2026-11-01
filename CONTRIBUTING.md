# Contributing

Thanks for helping! Open an issue first for anything bigger than a small fix,
so we can agree on the approach.

## Setup

Stable Rust (edition 2024) and Docker for the image build.

## Tests and linters

```sh
cargo fmt --all --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all-targets --all-features --locked
docker build -t rdp-proxy:dev .
```

CI runs exactly these commands on every push to `main` and on pull requests.
Do not commit real addresses, keys or `.env` files; use the documentation ranges
(`192.0.2.0/24`, `198.51.100.0/24`, `203.0.113.0/24`) in examples and tests.

## Commit messages

Use [Conventional Commits](https://www.conventionalcommits.org/):

```
feat: make health address configurable
fix(client): retry on QUERY_LIMIT_EXCEEDED for list calls
docs: add systemd example
```

Types: `feat`, `fix`, `docs`, `test`, `refactor`, `perf`, `build`, `ci`, `chore`.
Keep one logical change per pull request.
