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
| 2 (2026-10-18) | 6 MB | 3.9 MiB | 40 KB |
| 3b (2026-10-18) | 9 MB | 5.7 MiB (after processing a picture) | 55 KB |
| 7 / 1.0 (2026-10-18) | 10 MB | 4.8 MiB | 54 KB (initial; pages and sheets load on demand) |

Request times (release build on the development machine, kept-alive connection, a user with
3 shops, 6 lists, 240 items, 300 catalogue entries and a 60-wish wishlist):

| Request | Median | p95 |
|---|---|---|
| Tick an item (`PATCH /items/{id}`) | 2.0 ms | 2.5 ms |
| Quick add (`POST /lists/{id}/items`) | 3.3 ms | 4.3 ms |
| `GET /summary` | 2.8 ms | 4.4 ms |
| Catch up, nothing new (`GET /sync?since=<rev>`) | 4.4 ms | 6.3 ms |
| Everything at app start (`GET /sync?since=0`) | 22.6 ms | 30.2 ms |

The full load at app start is the one heavy request (it happens once per start); a Raspberry
Pi 4 is a few times slower.

## Releasing
1. Move the `[Unreleased]` notes in `CHANGELOG.md` under the new version and date; set the
   version in `Cargo.toml` (workspace) and `web/package.json`.
2. Commit, then tag and push the tag: `git tag v1.2.3 && git push origin v1.2.3`.
3. The *Release* workflow builds the image for amd64 and arm64 (cross-compiled, see the
   `Dockerfile`), pushes it to `ghcr.io/laurits7/shoppingcart` (`1.2.3`, `1.2`, `latest`) and
   creates the GitHub release with the changelog section and a `docker-compose.yml` pinned to
   that version.
4. The GHCR package of a private repository is private: make it public in the package's
   settings if others should pull it without signing in.

Between releases, every push to `main` that passes CI also publishes
`ghcr.io/laurits7/shoppingcart:edge` (and `:sha-<commit>`) for amd64 and arm64, for trying the
latest changes; `latest` only ever points at a release.
