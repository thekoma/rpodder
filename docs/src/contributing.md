# Contributing

rpodder is open source and contributions are welcome!

## Development setup

```bash
git clone https://github.com/thekoma/rpodder.git
cd rpodder

# Start PostgreSQL + dev server with live reload
docker compose up -d

# Or build locally
cd web && bun install && bun run build && cd ..
cargo build
cargo test
```

## Running tests

```bash
cargo test                           # all tests (~90)
cargo test -p rpodder-core           # single crate
cargo test -p rpodder-db             # DB tests (uses in-memory SQLite)
cargo clippy --workspace -- -D warnings  # lint
cargo fmt --all -- --check           # formatting
```

All tests use in-memory SQLite databases — no external services needed.

## Code style

- **Clippy clean** — CI enforces `clippy -- -D warnings`
- **Formatted** — `cargo fmt` with default settings
- **No compile-time DB macros** — use `query_as` with `FromRow`, not `query!`
- **Idempotent migrations** — every SQL migration must be safe to re-run

## Adding a new feature

1. If it needs new data, add a migration in `migrations/postgresql/` and `migrations/sqlite/`
2. If it needs a new domain type, add it to `rpodder-core/src/types.rs`
3. If it needs DB access, add a trait method to `rpodder-core/src/repo.rs` and implement it in both `postgres.rs` and `sqlite.rs`
4. Add the HTTP handler in `rpodder-server/src/routes/`
5. Register the route in `main.rs`
6. Update the web UI in `web/src/`
7. Add tests

## Project philosophy

### About vibe coding

rpodder is built through human-AI collaboration. We use Claude (Anthropic) as a development partner — not as a code generator that we blindly copy-paste from, but as a collaborator that we discuss architecture with, review output from, and iterate on together.

We believe this is a legitimate and increasingly common way to build software. The code is tested, reviewed, and functional. We're transparent about our process because we think the industry benefits from honesty about how software is actually made.

### Design decisions

- **Why Rust?** — performance, safety, single binary deployment, great async ecosystem
- **Why axum?** — lightweight, composable, tower middleware ecosystem
- **Why Svelte?** — small bundles, reactive without virtual DOM, great DX
- **Why both PG and SQLite?** — PG for multi-user production, SQLite for Raspberry Pi self-hosting
- **Why not MySQL?** — two databases is enough complexity. PG and SQLite cover all use cases
- **Why UUID v7?** — time-sortable, globally unique, no sequence coordination needed

## Filing issues

Please report bugs and feature requests at [github.com/thekoma/rpodder/issues](https://github.com/thekoma/rpodder/issues).

## License

By contributing, you agree that your contributions will be licensed under AGPL-3.0-or-later.
