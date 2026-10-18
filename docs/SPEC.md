# ShoppingCart: Project Spec

This document is the source of truth for what ShoppingCart is and how it is built. Section
numbers (§4.2 etc.) are referenced from [`WORKPLAN.md`](WORKPLAN.md) and
[`DECISIONS.md`](DECISIONS.md). Build **one chunk at a time** (see the work plan), keep every
chunk runnable, and ask before contradicting this spec.

## 1. Vision

A small, self-hosted **shopping list and wishlist** web app for a household. It is deployed
like its sibling [Streamline](https://github.com/Laurits7/Streamline): one `docker compose up -d`,
then open `http://IP:PORT` on any phone, tablet or computer and sign in.

- **Shopping** tab: lists per **shop**, each list split into **categories** (Dairy, Meat,
  Vegetables…), everything ordered by **drag and drop**, a **default list** and a **staples**
  template per shop.
- **Wishlist** tab: my own wishlist split into **topic sections**, plus **gift lists** for other
  people that the person themselves never sees, with "I'll get this" claims.
- **Private by default, shareable on purpose**: lists belong to a person; the owner shares a
  list with a household or invites individual people.
- **Connects** to [Mealie](https://mealie.io) (pulls its shopping list) and to Streamline
  (exposes what needs buying, so Streamline can create a "Go shopping" task), through a
  documented, token-authenticated API.

## 2. Goals and non-goals

**Goals**
- **Lean**: tiny image, tiny memory footprint, tiny frontend. Suitable for a Raspberry Pi next
  to Mealie and Streamline.
- **Very responsive**: every tap is applied instantly (optimistic UI), live updates between
  devices and people, no full page loads, no spinners for local actions.
- **Secure by default**: login required for everything, private-by-default data, server-side
  permission checks on every read, write and live event (§7).
- **Usable in the shop**: big touch targets, one-handed ticking, tolerant of a flaky mobile
  connection in the store (§6.7).
- **Documented**: user and developer docs in an MkDocs site, the API in OpenAPI.

**Non-goals (v1)**
- Native mobile apps (the PWA is the phone app).
- Public multi-tenant hosting, self-sign-up, social features, e-mail.
- Prices, budgets, receipts, loyalty cards, store inventory or pantry tracking.
- Writing back to Streamline, or two-way sync with Mealie (D-3).

## 3. Deployment

- One container built from a multi-stage `Dockerfile`; `docker-compose.yml` at the repo root
  builds it (or uses the prebuilt image from GHCR once releases exist).
- Persistent data in a mounted `./data` folder: one **SQLite** file, `secret.key` and
  `images/` (product pictures, §6.4b). Backup
  means copying the folder (an in-app backup comes in Phase 7).
- Configuration through environment variables (§3.1); nothing required except the volume.
- Must work over plain HTTP on a LAN, and equally behind an HTTPS reverse proxy (Caddy,
  Traefik, Tailscale serve). Never hard-code scheme or host. Service worker, install prompt and
  `Secure` cookies switch on automatically when served over HTTPS.
- Runs as a non-root user from a `scratch` image, works with a read-only root filesystem
  (`read_only: true` in compose), only `/data` is writable.
- Outgoing internet access is only needed for picture/product lookups (§6.4b) and can be
  switched off with `PRODUCT_LOOKUP=off`; everything else works fully offline on a LAN.

### 3.1 Environment variables

| Variable | Default | Purpose |
|---|---|---|
| `PORT` | `3100` | HTTP port inside the container (3100 so it can sit next to Streamline's 3000) |
| `DATA_DIR` | `/data` | Database, `secret.key`, backups |
| `LOG_LEVEL` | `info` | `tracing` filter |
| `PUBLIC_URL` | unset | Base URL used in links handed to other apps (e.g. Streamline); otherwise taken from the request |
| `TRUST_PROXY` | `false` | Trust `X-Forwarded-For/-Proto` from a reverse proxy |
| `INITIAL_ADMIN_USER` / `INITIAL_ADMIN_PASSWORD` | unset | Create the first admin on start (otherwise the web UI asks on first visit) |
| `SECRET_KEY` | from `data/secret.key` | Key for encrypting stored integration tokens; generated on first start |
| `SESSION_DAYS` | `30` | Sliding session lifetime |
| `PRODUCT_LOOKUP` | `openfoodfacts` | Where product pictures/brands are looked up (§6.4b); `off` disables all lookups |
| `MAX_UPLOAD_MB` | `8` | Largest picture upload accepted |

## 4. Tech stack

The same stack as Streamline, so both apps share conventions and the owner maintains one way
of working.

- **Backend: Rust**: Axum 0.8, Tokio, **SQLite via sqlx** with migrations, `argon2` for
  passwords, `utoipa` for the OpenAPI document, `ts-rs` to generate TypeScript types, `reqwest`
  (rustls, `ring`) for outgoing integration and product-lookup calls, the `image` crate (only
  the JPEG/PNG/WebP decoders and the JPEG encoder) to resize pictures into small JPEG files (D-39). A static **musl** binary.
- **Frontend: Svelte 5 + Vite as a static SPA** (no SvelteKit, see Streamline D-25), with a tiny
  history router, a client-side store and an outbox for optimistic writes. Built assets are
  **embedded in the binary** (`rust-embed`), so it is one process and one container.
- **Drag and drop** is hand-rolled on Pointer Events (mouse, touch and pen, with a long-press
  to start on touch, auto-scroll and keyboard reordering), not a library. Streamline's
  `web/src/lib/dnd.svelte.ts` is the starting point.
- **Real time**: Server-Sent Events (`/api/v1/events`), filtered per user by the same
  permission rules as reads (§7.3).
- **Docs**: MkDocs + Material in `docs/`, `mkdocs build --strict` in CI, published to GitHub
  Pages.
- Prefer boring, well-maintained dependencies; check crate/npm maturity before adding one and
  record notable choices in `DECISIONS.md`.

### 4.1 Resource targets (measure at every phase gate)

| What | Target |
|---|---|
| Docker image | < 20 MB |
| Idle memory (whole container) | < 20 MB |
| Frontend initial load | < 60 KB gzip (JS + CSS) |
| API latency on a Pi 4, typical request | < 10 ms server time |
| Perceived latency of a tap | < 100 ms (optimistic) |

### 4.2 API-first

- The web app is just one client of a **versioned HTTP API** at `/api/v1`. No business rule
  lives only in the frontend.
- Auth: session cookie for the browser; `Authorization: Bearer <token>` with per-client
  **API tokens** (created and revoked in Settings, shown once, stored hashed, optional
  read-only scope, §7.2).
- **ULID ids** for all entities; clients may choose ids on create, and retrying a create with
  the same id is idempotent (needed by the outbox, §6.7).
- Every row has `created_at`, `updated_at`, `deleted_at` (tombstone) and a global `rev`
  counter, so clients can ask `GET /sync?since=<rev>` for everything changed.
- OpenAPI at `/api/v1/openapi.json`, human-readable viewer at `/api/docs`; TypeScript types
  generated from the Rust structs and committed.
- Errors are RFC 9457 problem documents.

## 5. Core concepts and data model

Names are indicative. All tables have `id` (ULID), `created_at`, `updated_at`, `deleted_at`,
`rev` unless noted.

### 5.1 People
- **User**: `username` (unique, case-insensitive), `display_name`, `password_hash` (argon2id),
  `is_admin`, `locale`. Created by an admin (no self-sign-up).
- **Session** (no `rev`): hashed token, user, `expires_at`, `last_seen_at`, user agent.
- **ApiToken** (no `rev`): user, name, hashed token, `scope` (`read` | `write`),
  `last_used_at`, `revoked_at`.
- **Household**: a named group of users (e.g. "Home", "Parents"). A user can be in several.
- **HouseholdMember**: household, user, `role` (`owner` | `member`). Owners rename the
  household and add/remove members; admins can manage every household.

Households are **share targets**, not owners: nothing is visible to a household unless its
owner shared it there (D-5).

### 5.2 Shopping
- **Shop**: owner (user), `name`, optional `color`/`icon`, `position`, `default_list_id`.
  Shops are personal: each user arranges their own shops.
- **List**: `shop_id`, owner (user, the shop's owner), `name`, `position`, `archived_at`.
  Exactly one list per shop is the **default list** (§6.2). Lists are the unit of sharing.
- **Category**: `list_id`, `name`, `position`. Categories belong to a list, so a shared list
  carries its own layout. A new list starts with a copy of the categories of its shop's default
  list (D-7). Items without a category show under *Other* at the end.
- **Item**: `list_id`, `category_id?`, `catalog_item_id?`, `name`, `amount` (decimal, default 1),
  `unit?` ("kg", "g", "l", "pack"…; empty means pieces), `step?` (overrides the +/- step),
  `note`, `position`, `checked_at?`, `checked_by?`, `added_by`, `source`
  (`manual` | `staple` | `mealie` | `api`), `external_ref?` (e.g. the Mealie item id).
- **Staple**: `shop_id`, `catalog_item_id?`, `name`, `amount`, `unit?`, `category_name?`, `position`. The shop's
  template of usual purchases (§6.3).
- **CatalogItem**: the **database of previously used items**, per user: `name`,
  `normalized_name` (unique per user: lower-cased, trimmed, diacritics kept), last used
  category name, usual `amount`/`unit`/`step`, `use_count`, `last_used_at`, optional `shop_id`
  of the shop it was last bought at, `hidden`. Filled automatically whenever the user adds an
  item anywhere; editable in Settings (§6.4a). Powers suggestions, auto-categorising and
  the frequent-items chips.
- **Product**: a specific, branded version of a catalogue item: `catalog_item_id`, `brand`
  ("Alma", "Farmi"), `name` ("Piim 2.5% 1 l"), `barcode?` (EAN/UPC), `image_id?`,
  `source` (`manual` | `openfoodfacts`), `source_ref?`, `position`. A catalogue item can have
  any number of products and one **preferred** product (optionally per shop:
  **ProductPreference** `catalog_item_id`, `shop_id?`, `product_id`).
  Items and staples get an optional `product_id`: empty means "any milk", set means "milk by X".
- **Image** (no `rev`): owner, `sha256` (dedup), `mime`, `width`, `height`, `bytes`, stored as
  `data/images/<sha256>.jpg` (thumbnail ≈ 160 px) and `…-l.jpg` (≈ 640 px, D-39). Never the
  original upload. Also usable as a picture for a catalogue item without a brand.

### 5.3 Wishlists
- **Person**: owner (user), `name`, optional `user_id` (when the person has an account here).
  Used as gift recipients. A user's own wishlist doesn't need one.
- **Wishlist**: owner (user), `kind` (`own` | `gift`), `recipient_person_id` (for `gift`),
  `title`, `position`.
- **WishSection**: `wishlist_id`, `name` (a topic: "Books", "Kitchen", "Hiking"), `position`.
- **WishItem**: `wishlist_id`, `section_id?`, `title`, `url?`, `price?` (decimal + currency),
  `note`, `priority` (0–2), `position`, `received_at?`.
- **Claim**: `wish_item_id`, `user_id`, `status` (`planned` | `bought`), `note`. At most one
  active claim per item.

### 5.4 Sharing
- **Share**: `resource_type` (`list` | `wishlist`), `resource_id`, `principal_type`
  (`user` | `household`), `principal_id`, `role` (`editor` | `viewer`), `created_by`.
  A household share includes the household's current **and future** members.
- **Event log** (no tombstones): `rev`, `kind`, `resource`, affected-list/wishlist id, used by
  `/sync` and SSE fan-out.

### 5.5 Integrations
- **Integration**: owner (user), `provider` (`mealie`), `settings` (JSON), `secret`
  (encrypted with `SECRET_KEY`, XChaCha20-Poly1305, never returned to clients),
  `last_sync_at`, `last_error`, `enabled`.
- **ExternalLink** (no `rev`): integration, external id, local item id. Prevents duplicates.

## 6. Feature requirements

### 6.1 Shops and lists
- Create, rename, recolour, reorder (drag) and delete shops. Deleting asks for confirmation and
  says how many lists and items go with it.
- A shop holds one or more lists (e.g. "Weekly", "Party on Saturday"). Lists can be created,
  renamed, reordered, archived and deleted. One of them is the **default**.
- The Shopping tab is a **summary page**: every shop with its lists, each list as a card with
  its open items (tick and −/+ in place), its own quick add and the cart folded into one line
  with *Finish shopping*. A list's title opens it full screen, grouped by category (D-34).
  Lists shared with me appear under their owner's shop name, marked as shared.
- Move an item (or a selection of items) to another list, including a list of another shop.

### 6.2 Default list
- Each shop's default list is where items land when no list is chosen:
  `POST /shops/{id}/items` in the API, Mealie imports (§6.10).
- Creating a shop creates its default list ("Shopping list"); any list can be made default;
  the default list can't be deleted until another is made default.

### 6.3 Staples
- Each shop has a **staples** template: the things usually bought there (milk, bread, coffee).
- "Add staples" on a list shows the staples as a checklist, pre-ticking those not already open on
  the list; one tap adds them, with category and quantity.
- Any item can be turned into a staple from its menu; staples are reordered by drag.

### 6.4 Items and categories
- **Quick add** box at the top of the list: type, Enter, next. A trailing amount and unit is
  parsed ("milk 2", "flour 1 kg"). While typing, **suggestions from the item catalogue**
  (§6.4a) appear instantly; tapping one adds it at once, tapping it again bumps its amount.
  A catalogue item drops into its
  remembered category automatically.
- **Categories**: create, rename, reorder (drag), collapse, delete (items move to *Other*).
  New lists get sensible starter categories only when there is nothing to copy (D-7).
- **Ticking**: one tap ticks an item; ticked items move to an *In the cart* section at the
  bottom (still visible, untick with one tap). **Finish shopping** removes ticked items from
  the list (they stay in the catalogue), with undo.
- **Amount stepper**: every item shows its amount with **−** and **+** buttons (large touch
  targets). The step is 1 for pieces/packs, 0.5 for kg/l, 100 for g/ml, or the item's own `step`.
  Pressing − at the smallest step asks to remove the item (or removes it with undo). Tapping the
  amount allows typing an exact value and unit.
- Edit name, amount, unit, note, category inline; swipe or menu to delete, with undo.
- Duplicate guard: adding an item that is already open on the list **adds to its amount**
  instead of creating a second row (with a toast "Milk: 2 → 3", undo).

### 6.4a Item catalogue (previously used items)
- Every item a user adds (by typing, from staples, Mealie or the API) is recorded in their
  **catalogue** with its category, usual amount/unit and how often and how recently it was used.
- **Suggestions while typing**: matches by prefix and word start first, then substring, ranked
  by frequency and recency (and boosted for the current shop). Results come from a client-side
  copy of the catalogue, so they appear on every keystroke with no network round trip; the
  server endpoint `GET /catalog?q=` exists for API clients.
- **Frequent items chips**: with an empty quick-add box, the most frequently bought items for
  this shop that are not already on the list are shown as chips; one tap adds.
- **Manage** in Settings → Item catalogue: rename, change default category/unit/step, merge
  duplicates ("Tomato" + "Tomatoes"), hide or delete entries.
- The catalogue is personal. On a shared list, suggestions come from your own catalogue, and
  what you add goes into your own catalogue (Q-9).

### 6.4b Products, brands and pictures
The same thing often comes from different producers: *milk by Alma* or *milk by Farmi*.
- A catalogue item ("Milk") can have **products** (brand + name + picture, optionally a
  barcode). One can be marked **preferred**, globally or for a particular shop ("at Rimi I buy
  Farmi").
- On a list, an item is either **generic** ("Milk") or **pinned to a product** ("Milk · Alma 2.5%"),
  shown with its **picture** as a small thumbnail; tap the thumbnail for a larger picture, which
  helps whoever does the shopping pick the right one. A generic item with a preferred product
  shows that product's picture, labelled as preferred.
- **Choosing a brand**: from an item's menu (or long-press on a suggestion) a sheet shows the
  catalogue item's products with pictures; pick one, add a new one, or keep it generic. Two
  products of the same item can be on a list at once (e.g. two different milks).
- **Suggestions while typing** (§6.4a) include products as their own rows with thumbnails
  ("Milk · Alma", "Milk · Farmi") under the generic entry.
- **Pulling pictures and brands**: "Find pictures" on a catalogue item or product searches
  **Open Food Facts** (free, open product database with photos and brands) by name, or by
  barcode, and offers the results to pick from; the chosen picture is downloaded **by the
  server**, resized and stored locally, so the browser never loads third-party images and the
  list works offline. The source is recorded (Open Food Facts pictures are CC BY-SA; the
  attribution is shown on the large view).
- **Own pictures**: upload or take a photo with the phone camera (`<input capture>`); it is
  resized on the server the same way. This is the fallback when a local product isn't in
  Open Food Facts.
- **Barcode** (HTTPS only, where the browser supports `BarcodeDetector`): scan a product's
  barcode in the shop to look it up or to add it to the list; on browsers without it, the
  barcode can be typed.
- Mealie/API imports stay generic unless the name matches a product exactly.
- Lookups are optional (`PRODUCT_LOOKUP=off`); if Open Food Facts is unreachable, only the
  "Find pictures" button is affected.
- Implementation note: read Open Food Facts' current API docs (search and product-by-barcode
  endpoints, required `User-Agent`, rate limits) at implementation time rather than relying on
  remembered endpoints; cache lookups for a day.

### 6.5 Drag-and-drop ordering
- Drag items within a category, between categories, and into *Other*; drag categories, lists,
  shops, staples, wish sections and wish items.
- Works with mouse, touch (long-press to lift) and keyboard (pick up with Space, move with
  arrows, drop with Space; announced to screen readers).
- Order is stored as **fractional-index position keys** (strings), so a move writes one row and
  two people reordering at once never corrupt the order. The algorithm lives in the domain
  crate with tests; keys are rebalanced only when they grow long.

### 6.6 Live sync between devices and people
- Changes appear on every device of everyone who can see the list within about a second (SSE).
- On reconnect the client catches up with `/sync?since=<rev>`; if too far behind, it reloads.
- Concurrent edits: last write wins per field; a tick and an edit never lose each other.

### 6.7 Flaky connections in the shop
- Writes go to a persistent **outbox** (IndexedDB, falling back to memory) and are applied to the
  local store immediately; the outbox retries in order with the client-chosen ids, so retries
  are idempotent.
- The last loaded lists stay usable when the connection drops; a small banner says "offline,
  N changes waiting". When served over HTTPS, a service worker also keeps the app shell
  available offline.

### 6.8 Sharing lists
- A list is **private to its owner** by default.
- The owner can share it with a **household** (every member, now and later) or invite
  individual **users**, each as **editor** (add, tick, edit, reorder items and categories) or
  **viewer** (read only, D-8).
- Only the owner can rename, delete, archive or re-share the list, and change its default status.
- The share sheet lists who can see the list and why ("via household Home"). Revoking a share
  takes effect immediately, including on open live connections.
- Someone with access can "leave" a list shared with them individually.

### 6.9 Wishlists and gift lists
- **Wishlist** tab with two parts: **My wishlists** and **Gift ideas**.
- **My wishlists**: one or more own wishlists, each with **topic sections**. Items have a title,
  optional link, price, note and priority. Private by default; shareable like lists (§6.8),
  typically with a household so family can see what I'd like.
- **Gift ideas**: per **person** (a household member or anyone by name, e.g. "Grandma"), a gift
  list with its own sections. Shareable with households/users like any wishlist.
- **The recipient never sees a gift list about them**, even if it is shared with a household
  they belong to, and gets no hint that it exists. This is enforced server-side (§7.3).
- **Claims**: anyone who can see an item (other than the wish's owner on an own wishlist, or the
  recipient) can mark it **"I'll get this"** and later **bought**. Others see that it is taken
  (and by whom). On my **own** wishlist I never see claims, so surprises stay surprises.
- Mark an item **received** to move it to a *Received* archive; delete or restore.
- Copy an item to another wishlist (e.g. from a gift list into my own); lists stay separate.

### 6.10 Mealie integration (pull)
- Per user, in Settings → Integrations: Mealie base URL, Mealie API token, which Mealie shopping
  list to watch, and the **target**: a shop (its default list) or a specific list.
- Every 5 minutes (and on "Sync now") the server reads unchecked items of the watched Mealie
  list, adds them to the target list (`source = mealie`, linked via ExternalLink so nothing is
  imported twice), then **ticks them in Mealie** so Mealie's list works as an inbox (D-3).
- Mapping: Mealie's food/quantity/unit/note → name, amount and unit (display text as fallback); a Mealie **label**
  maps to a category of the same name in the target list (created if missing, optional).
- Failures (Mealie down, bad token) never block the app; Settings shows the last error and
  last successful sync.
- Implementation note: read the running Mealie instance's `/docs` (OpenAPI) for current
  endpoints and auth, since they changed between v1 and v2+; develop against a pinned Mealie
  version in a dev compose profile.

### 6.11 Streamline integration
**ShoppingCart side (this repo):**
- A read-scoped API token is enough to call `GET /api/v1/summary`: for each list the token's
  user can see, the shop, list, open item count, item names (first N), `updated_at`, and a
  `url` deep link (built from `PUBLIC_URL`).
- Optional: `POST /api/v1/lists/{id}/finish` (write scope) so another app can mark a run done.

**Streamline side (spec only here; built later in the Streamline repo, §9):**
- A ShoppingCart provider in Streamline's integration framework (its §6.15): base URL + token,
  polled like Mealie. While a chosen list (or any list of a chosen shop) has at least N open
  items, Streamline keeps **one** open task "Go shopping: <Shop> (N items)" with the items in
  its details and a link back; when the list is emptied it completes the task automatically.
  Default task type *carry on*; place = the shop, if Streamline places are used.

### 6.12 Households and users
- **Admin** (first user) manages users: create, reset password, make admin, deactivate.
- Admins and household owners create households and add or remove members (existing users).
- Leaving a household removes access to everything shared via that household, immediately.
- Settings for a user: display name, password (changing it signs out other sessions and revokes
  API tokens), sessions list with "sign out", API tokens, integrations, theme (system/light/dark).

### 6.13 Web app and UI
- Two main tabs, **Shopping** and **Wishlist**, plus **Settings**; bottom tab bar on phones,
  sidebar on wide screens.
- Mobile first; large touch targets in the list; works one-handed.
- Light and dark theme (system by default); accessible (WCAG AA contrast, keyboard use, labels,
  reduced motion respected).
- **PWA**: manifest + icons always; service worker and install prompt over HTTPS.
- English UI in v1, with strings kept in one place so translations (e.g. Estonian) can be added
  (Q-6).

### 6.14 Backups and export (Phase 7)
- Nightly SQLite online backup into `data/backups/`, keeping the last N; download from Settings
  (admin).
- Per-user JSON export of their own lists and wishlists.

## 7. Security

### 7.1 Authentication
- Passwords hashed with **argon2id** (OWASP parameters); minimum length 10; constant-time
  comparisons; no user enumeration in login errors.
- Sessions: random 256-bit token in an `HttpOnly`, `SameSite=Lax` cookie (`Secure` over HTTPS),
  only its SHA-256 hash stored; sliding expiry; logout deletes the row.
- **Login rate limiting** per IP and per username (exponential backoff), honest about proxies
  only when `TRUST_PROXY` is set.
- First-run setup is only possible while no user exists.

### 7.2 API tokens
- 256-bit random, prefixed `sc_` so they are recognisable in leaks; shown once; stored as
  SHA-256; `read` or `write` scope; revocable; `last_used_at` recorded.
- Bearer tokens are not accepted from query strings (except the SSE endpoint, which needs a
  short-lived signed ticket instead).

### 7.3 Authorisation
- **Every** query that reads or writes shopping or wishlist data goes through one visibility
  module (owner, user share, household share, gift-recipient exclusion). No ad-hoc checks in
  handlers. Resources a user can't see return 404, not 403 (no existence leak).
- Gift lists: the recipient (`Person.user_id`) is excluded from every read path: lists, items,
  claims, search, counts, `/sync`, SSE events, exports. Claims on own wishlists are filtered
  out for the owner the same way. These rules have dedicated tests.
- SSE events are only delivered to users who can see the affected resource *at delivery time*.

### 7.4 Web hardening
- CSRF: `SameSite=Lax` cookies plus a required `X-Requested-With`/JSON content-type on every
  state-changing request; reject cross-site `Origin`.
- Strict **Content-Security-Policy** (`default-src 'self'`, no inline scripts), `X-Content-Type-Options`,
  `Referrer-Policy: same-origin`, `frame-ancestors 'none'`, HSTS only over HTTPS.
- Picture uploads: size limit (`MAX_UPLOAD_MB`), allowed types sniffed from content (JPEG,
  PNG, WebP only, never SVG), decoder dimension/memory limits, always re-encoded (which
  strips EXIF/GPS), served from our origin with `Content-Type` set by us and
  `X-Content-Type-Options: nosniff`, behind the same visibility checks as the data that uses them.
- Product lookups go only to the configured provider host, server-side, with timeouts and size
  limits; nothing personal is sent (just the search text or barcode).
- Request body size limits; input length limits on every text field; URLs in wish items only
  `http(s)` and rendered with `rel="noopener noreferrer"`.
- Outgoing integration requests: timeouts, response size limits, no redirects to other hosts;
  LAN targets are allowed on purpose (self-hosted Mealie), which is documented.
- Integration secrets encrypted at rest; never logged; never sent back to the browser.
- `cargo audit`/`npm audit` in CI; dependencies kept minimal.

## 8. Testing
- Domain crate (pure logic, unit tests): fractional indexing, amount/unit parsing, +/- step
  rules, item-name normalisation, catalogue ranking, product preference resolution,
  Open Food Facts response mapping, Mealie item mapping, visibility rules expressed as pure functions.
- Server integration tests against an in-memory SQLite: auth, tokens and scopes, sharing and
  revocation, **gift-recipient exclusion on every endpoint**, claims hidden from owners,
  idempotent creates, `/sync` correctness.
- Web: unit tests (vitest) for the store, outbox and DnD maths; `svelte-check` with strict TS.
- CI: `cargo fmt --check`, `clippy -D warnings`, tests, web check/test, bundle size check,
  `mkdocs build --strict`, docker build.

## 9. Build phases

See [`WORKPLAN.md`](WORKPLAN.md) for the chunks. In short:

0. **Spec and docs skeleton** (this document, work plan, decisions, MkDocs site).
1. **Foundation**: workspace, Docker/compose, SQLite + migrations, first-run setup, login,
   sessions, users, households, API tokens, OpenAPI, security headers, UI shell, CI.
2. **Shopping core**: shops, lists, default list, categories, items, quick add, ticking,
   amount stepper, finish shopping, staples, item catalogue with suggestions.
3. **Drag and drop** everywhere, fractional indexing, move between lists.
3b. **Products and pictures**: brands per catalogue item, preferred products, picture upload
   and resizing, Open Food Facts lookup, barcode scanning.
4. **Live sync and sharing**: SSE, `/sync`, outbox, list sharing with households and users.
5. **Wishlists**: own wishlists with sections, persons, gift lists, claims, privacy rules.
6. **Integrations**: summary API for Streamline, Mealie pull, docs for both; then the
   Streamline-side provider in the Streamline repo.
7. **PWA and polish**: service worker, backups, export, accessibility pass, performance
   measurements, release workflow (GHCR image, GitHub Pages docs), v1.0.

## 10. Working agreements for Claude Code
- Read `CLAUDE.md`, this spec, the work plan and the decisions before starting a session.
- One chunk (or one small phase) per session; tick its checkboxes in the work plan when it lands.
- Record answers and deviations in `DECISIONS.md` (`D-n`). Don't start a chunk marked
  `needs Q-x` until that question has a decision.
- Update the user docs in `docs/` in the same chunk as the feature; keep `README.md` short.
- Gate commits on real exit codes; never commit or push without the owner's explicit OK.
