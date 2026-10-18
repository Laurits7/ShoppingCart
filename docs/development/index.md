# Developing

Rust (stable, 1.88+) and Node 22+. Same layout and habits as Streamline.

```sh
make dev-server   # Rust API on :3100 with ./data
make dev-web      # Vite on :5173, proxies /api to :3100
make test         # cargo tests + web checks and tests
make check        # fmt, clippy -D warnings, svelte-check
make types        # regenerate TypeScript types from Rust (ts-rs)
make up           # docker compose up -d --build
```

## Layout
- `crates/domain`: pure logic with unit tests (ordering keys, quantity parsing, visibility rules).
- `crates/server`: the Axum binary (API, auth, SSE, jobs, embedded web app).
- `migrations/`: SQLite migrations, run on start.
- `web/`: the Svelte 5 SPA.
- `docs/`: this site, plus the [spec](../SPEC.md), [work plan](../WORKPLAN.md) and
  [decisions](../DECISIONS.md).

## How work is done
The work plan splits everything into chunks that fit one session. Pick the next unticked chunk,
read the spec sections it references, build it with tests and docs, tick it, record decisions.

## Docs
```sh
python3 -m venv .venv && .venv/bin/pip install -r docs/requirements.txt
.venv/bin/mkdocs serve           # preview
.venv/bin/mkdocs build --strict  # what CI runs
```

## Measurements
Filled in at each phase gate (targets in spec §4.1).

| Phase | Image | Idle RSS | Initial JS+CSS (gzip) |
|---|---|---|---|
| 1 (2026-10-18) | 5 MB | 2.9 MiB | 25 KB |
