# Decisions

Short log of confirmed decisions and deviations from [`SPEC.md`](SPEC.md). Newest at the bottom.
Format: **D-n · date · topic**: decision. *Why / notes.*

## Owner's answers (2026-10-18)

- **D-1 · 2026-10-18 · Default list**: Both meanings: each shop has a **default list** (where
  quick-add, the API and Mealie put items) **and** a **staples** template. *Spec §6.2, §6.3.*
- **D-2 · 2026-10-18 · Gift-list privacy**: A gift list is hidden from its recipient; everyone
  else who can see it can claim items ("I'll get this") to avoid duplicate gifts. *Spec §6.9, §7.3.*
- **D-3 · 2026-10-18 · Mealie**: Pull only: poll a chosen Mealie shopping list, import unchecked
  items into a shop's list, tick them in Mealie (Mealie as inbox). No two-way sync, no recipe
  import in v1. *Spec §6.10.*
- **D-4 · 2026-10-18 · Streamline**: ShoppingCart exposes a summary API; the Streamline side
  ("Go shopping: <Shop>" task) is specified here and built later in the Streamline repo.
  *Spec §6.11.*
- **D-5 · 2026-10-18 · Ownership and sharing**: Shopping lists belong to a **user** and are
  private by default. The owner can share a list with a household or invite individual users.
  Households are share targets, not owners. Wishlists follow the same model. *Spec §5.4, §6.8.*

## Open questions (WORKPLAN Part C)

All proposed defaults (Q-1 … Q-10) were accepted by the owner on 2026-10-18.

- **D-12 · Q-1 Ticked items**: move to *In the cart*; **Finish shopping** removes them (10 s undo).
- **D-13 · Q-2 Viewers**: read only, can't tick.
- **D-14 · Q-3 Own wishlist**: private by default.
- **D-15 · Q-4 Claims on gift lists**: visible with the claimer's name to everyone who can see the list except the recipient.
- **D-16 · Q-5 Mealie after import**: tick in Mealie (configurable).
- **D-17 · Q-6 Language**: English in v1, strings centralised for Estonian later.
- **D-18 · Q-7 Currency**: per-user setting, default EUR.
- **D-19 · Q-8 Accounts**: admin creates users; no sign-up or invite links in v1.
- **D-20 · Q-9 Catalogue**: per user.
- **D-21 · Q-10 Pictures**: Open Food Facts (on by default, `PRODUCT_LOOKUP=off`) plus own photos; products per user, visible to people on a shared list through its items.

## Architecture

- **D-6 · 2026-10-18 · Stack**: Same as Streamline: Rust (Axum, sqlx/SQLite), Svelte 5 + Vite
  SPA embedded in one static binary, `scratch` image, MkDocs Material. *The owner asked for Rust
  and leanness; reusing Streamline's stack and conventions keeps both apps maintainable alike.*
- **D-7 · 2026-10-18 · Categories per list**: Categories belong to a list (not to the shop), so
  a shared list carries its own layout. A new list copies the categories of its shop's default
  list; only a shop's very first list gets starter categories. *Because sharing is per list
  (D-5), shop-level categories would leak the owner's shop setup or be uneditable for guests.*
- **D-8 · 2026-10-18 · Share roles**: `editor` (everything on items and categories) and `viewer`
  (read only). Only the owner manages the list itself and its shares. *Confirmed by D-13.*
- **D-9 · 2026-10-18 · Default port 3100**: so it runs next to Streamline (3000) without
  remapping.
- **D-10 · 2026-10-18 · Amounts and item catalogue (owner request)**: Items have a numeric
  `amount` + optional `unit` (instead of free-text quantity) so −/+ buttons can step them; step
  by unit (1 / 0.5 kg,l / 100 g,ml) or per item. Every added item is recorded in a per-user
  **item catalogue** that drives instant client-side suggestions, frequent-item chips and
  auto-categorising, and can be managed in Settings. Adding a duplicate adds to its amount.
  *Spec §5.2, §6.4, §6.4a.*
- **D-11 · 2026-10-18 · Products, brands and pictures (owner request)**: A catalogue item can
  have several **products** (brand, name, barcode, picture) with a preferred one (optionally
  per shop); list items are generic or pinned to a product and show its picture. Pictures are
  pulled from **Open Food Facts** server-side, or uploaded/photographed, and stored resized in
  `data/images`. New Phase 3b. *Spec §5.2, §6.4b, §7.4.*

## Phase 1 (2026-10-18)

- **D-22 · Households**: any user can create a household and becomes its owner (the spec said
  admins and owners); owners and admins manage members, anyone can leave, and a household always
  keeps an owner (the longest-standing member is promoted). *Households only grant what is
  explicitly shared with them, so letting anyone create one carries no risk.*
- **D-23 · UI strings inline for now**: strings live in the components until a second language is
  actually added; extracting them is a mechanical chunk then. *Revises D-17; keeps the bundle and
  code simple meanwhile.*
- **D-24 · Container user**: the image runs as UID 10001; the compose file runs it as
  `${PUID:-1000}:${PGID:-1000}` with a read-only root filesystem, no capabilities and
  `no-new-privileges`, so `./data` (shipped empty in the repo) stays owned by the person who
  cloned it. The server refuses to start with a clear fix if `/data` isn't writable.
- **D-25 · Credentials need a browser session**: API tokens can't change passwords, list or revoke
  sessions, or create/revoke tokens or users; `read` tokens are refused on anything but
  GET/HEAD. Sessions slide (refreshed at most hourly) and expired ones are purged hourly.
- **D-26 · Argon2id parameters**: OWASP minimum (19 MiB, t=2, p=1), at most two hashes at once to
  bound memory on a Pi.

## Phase 2 (2026-10-18)

- **D-27 · Per-shop catalogue counts**: a small `catalog_shop_uses` table (catalogue item × shop:
  count, last use) next to the spec's single `shop_id`, because the frequent-item chips and the
  shop boost in suggestions need "how often at *this* shop", not just "where last".
- **D-28 · `GET /shopping` until `/sync`**: the web app loads the whole Shopping tab (shops,
  lists, categories, items, staples, catalogue) in one request. Phase 4's `GET /sync?since=0`
  replaces it for the web app; it stays as a convenience for API clients.
- **D-29 · Ordering keys now, moves in Phase 3**: `domain::order` (and its TS mirror) came over
  from Streamline in Phase 2 because every new row needs a position; new things are appended.
  Move/reorder endpoints and drag and drop stay in 3.1–3.3.
- **D-30 · Amount rules**: a typed amount without a unit means pieces ("milk 2"); a bare name
  takes the catalogue's usual amount and unit. The duplicate guard adds up the same unit and
  converts g↔kg and ml/cl/dl↔l; other unit mixes (1 l of milk + 2 milk) become a separate row.
  Ticked items never take duplicates. A merged create is remembered for a day
  (`item_merges`), so retrying it doesn't add the amount twice.
- **D-31 · Category names unique per list** (case-insensitive), so auto-categorising by name is
  unambiguous. The list view hides empty categories; collapsing is remembered per device.
- **D-32 · Failed changes**: until the outbox (Phase 4), a change the server refuses shows an
  error and the store reloads.
- **D-33 · Item source**: items added with an API token get `source = "api"`, from the web app
  `manual`, from staples `staple`.
- **D-34 · Summary page instead of a switcher (owner request)**: the Shopping tab is one page
  with every shop, each list as a card with its open items (tick and −/+ in place), its own
  quick add, and the cart folded into a line with *Finish shopping*. A list's title opens it full
  screen by category (`/list/{id}`). This replaces the drop-down shop switcher and "open the
  last-used list" (§6.1), and makes the separate quick-add in the switcher (§6.2) unnecessary.

## Phase 3 (2026-10-18)

- **D-35 · Moves name neighbours**: move endpoints take the sibling to go `after` or `before`
  and the server computes the ordering key (the client also computes one for the instant
  local update, which the answer replaces). Concurrent moves can't produce out-of-order keys
  this way. A scope is rebalanced to short evenly spaced keys (`order::spread`) when a key
  would pass 24 characters or two neighbours have equal keys.
- **D-36 · Item positions are per list**: categories only group items, so one key orders an
  item within its category. Moving to another list keeps a category of the same name, else
  *Other*; dropping into a category is remembered in the catalogue like choosing it.
- **D-37 · Grips**: lists, shops and categories drag by their grip only (so a press on a card
  doesn't lift it); items and staples also by the row (mouse) or a long-press (touch).
  Keyboard: Space picks up, arrows move (each step is applied at once), Space drops, Escape
  puts it back.
- **D-38 · Drag between lists on the summary page**: an item dropped on another list's card
  moves there. Only an open list shows empty categories as drop targets while dragging (cards
  would jump around otherwise).

## Phase 3b (2026-10-18)

- **D-39 · Pictures are JPEG, not WebP**: the `image` crate only writes lossless WebP, which is
  several times larger for photos; a lossy WebP encoder would mean a C library (libwebp). Pictures
  are stored as JPEG (quality ~80) at ≈160 and ≈640 px: pure Rust, small, shown everywhere.
  Transparency becomes white. Revises §5.2's `.webp` file names.
- **D-40 · Picture storage and access**: files are named by the SHA-256 of the original upload
  (the same picture is stored once); rows are per owner. A picture is visible to its owner and
  to anyone who can see a list with an item that shows it (pinned product, catalogue item, or a
  product of that item). Phone photos are turned upright from their EXIF tag before it is
  dropped. Pictures no product or catalogue item uses are deleted after a day.
- **D-41 · Open Food Facts usage**: name search uses search-a-licious
  (`search.openfoodfacts.org/search`), barcodes the v3 product API (v2 is deprecated); both
  read their docs on 2026-10-18. User-Agent `ShoppingCart/<version> (repository URL)`, no
  personal e-mail in it. We stay under their limits (8 searches and 12 product reads a minute
  per server; the provider allows 10 and 15) and cache results, including misses, for a day.
  Outgoing TLS is rustls with the `ring` provider and bundled Mozilla roots (`webpki-roots`),
  so no C crypto library and no system certificate store in the `scratch` image.
- **D-42 · Picker thumbnails through our server**: the result picker shows Open Food Facts
  pictures fetched by the server (`/lookup/picture/{code}`, only for cached results, served only
  after the bytes are sniffed as a picture), so the browser never loads third-party images.
- **D-43 · Barcode adds**: scanning (or typing) a barcode in quick add uses your product with that
  barcode, else looks it up and adds it as a new product of a new or existing catalogue item.
  Imports by API/Mealie match a product only by its exact name (§6.4b).
