# Changelog

All notable changes. Versions follow [semantic versioning](https://semver.org).

## [1.0.0] - unreleased

The first release.

### Shopping
- Shops with lists and templates (several per shop, on their own tab; start a list from one or
  add some of its items); categories per list; an inbox for items that came without a list; a
  summary page with every active list and its items.
- Quick add that understands amounts and units ("milk 2", "flour 1 kg"), −/+ steps per unit,
  adding a duplicate adds to its amount, ticked items greyed out at the end, Done (finish
  shopping) with undo.
- An item catalogue of everything you've added: instant suggestions, often-bought chips,
  remembered categories and amounts; rename, merge, hide and forget in Settings.
- Drag and drop everywhere (mouse, touch, keyboard), also between lists; Move up/down buttons.
- Brands and pictures: products per catalogue item, preferred brand everywhere or per shop,
  photos from the phone or from Open Food Facts (looked up and stored by the server), barcode
  scanning over HTTPS.

### Wishlists
- Own wishlists with topic sections, links, prices, priorities, a received archive.
- Gift lists for other people, never shown to the person they're for; "I'll get this" claims
  that the owner of a wishlist never sees.

### Together
- Private by default; share lists and wishlists with households or people, as editor or
  viewer; leaving and revoking take effect at once.
- Live updates between devices and people; changes made offline are kept and sent later; the
  app opens from the last saved copy without a connection.

### Integrations
- Mealie: a Mealie shopping list as an inbox (written against Mealie 3.29).
- A summary API for Streamline and dashboards; a documented, token-authenticated API
  (OpenAPI at `/api/docs`).

### Running it
- One small container (a static Rust binary with the web app built in), amd64 and arm64.
- Installable app (PWA) over HTTPS; nightly database backups, downloadable by admins; a
  per-user JSON export.
