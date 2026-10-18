# ShoppingCart

Self-hosted shopping lists (per shop, categories, drag-and-drop, default list + staples) and
wishlists/gift lists, with login, sharing and an API for Mealie and Streamline.
Rust/Axum + SQLite backend, Svelte SPA embedded in one binary/container (same stack as
`../Streamline`, which is a good reference for patterns: auth, tokens, SSE, dnd, Dockerfile).

- Spec: `docs/SPEC.md` (source of truth; section numbers like §6.8 refer to it).
- Plan: `docs/WORKPLAN.md` (phases → chunks with checkboxes). Work **one chunk/phase per
  session**, tick checkboxes as chunks land.
- Decisions: `docs/DECISIONS.md` (confirmed answers to open questions Q-x and deviations, `D-n`).
- User docs: the MkDocs site in `docs/` (`mkdocs.yml`). Update docs in the same chunk as the
  feature; keep `README.md` short. Check with `mkdocs build --strict`.
- Don't start a chunk marked `needs Q-x` until that question is decided in DECISIONS.md.
- Pure logic (ordering keys, amount parsing and +/- steps, catalogue ranking, visibility rules, Mealie mapping) goes in
  `crates/domain` with tests.
- All permission checks go through the server's `visibility` module. Gift lists must never
  reach their recipient on any path (reads, counts, /sync, SSE, export): test it.
- Gate commits on real exit codes, never on piped/grepped output. Never commit or push without
  the owner's explicit OK.
