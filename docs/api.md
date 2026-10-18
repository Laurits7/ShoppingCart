# API

All endpoints are under `/api/v1` and use JSON. The complete, always-current description is the
OpenAPI document at `/api/v1/openapi.json`, readable at `/api/docs` on your server.

## Authentication
Use `Authorization: Bearer <token>` with a token from **Settings → API tokens**. A **read**
token can only read; a **write** token can also change things. The web app itself uses a
session cookie.

```sh
curl -H "Authorization: Bearer sc_…" http://cart.local:3100/api/v1/summary
```

## Available now

| Endpoint | Purpose |
|---|---|
| `GET /health` | Liveness and version (no auth) |
| `GET/POST /setup` | Whether first-run setup is needed; create the first admin |
| `POST /auth/login`, `POST /auth/logout` | Sign in (sets the cookie) and out |
| `GET/PATCH /me`, `PATCH /me/prefs`, `POST /me/password` | Your profile, UI preferences, password |
| `GET /sessions`, `DELETE /sessions/{id}` | Where you're signed in (browser session only) |
| `GET/POST /tokens`, `DELETE /tokens/{id}` | API tokens: `{"name": "…", "scope": "read" \| "write"}` (browser session only) |
| `GET /users/directory` | Everyone's names, for sharing |
| `GET/POST /users`, `PATCH/DELETE /users/{id}` | User management (admins) |
| `GET/POST /households`, `PATCH/DELETE /households/{id}` | Households |
| `POST /households/{id}/members`, `PATCH/DELETE /households/{id}/members/{user_id}` | Members, roles, leaving |
| `GET /shopping` | Everything on the Shopping tab in one go: shops, lists, categories, items, staples, catalogue |
| `GET/POST /shops`, `PATCH/DELETE /shops/{id}` | Shops; creating one also creates its default list |
| `POST /shops/{id}/items` | Add an item to the shop's **default list** (handy for shortcuts and Home Assistant) |
| `GET/POST /lists`, `GET/PATCH/DELETE /lists/{id}` | Lists. `PATCH {"is_default": true}` makes a list the default, `{"archived": true}` archives it |
| `GET/POST /lists/{id}/categories`, `PATCH/DELETE /categories/{id}` | Categories (deleting one moves its items to *Other*) |
| `GET/POST /lists/{id}/items` | Items. Post `{"text": "flour 1 kg"}` or `{"name": "flour", "amount": 1, "unit": "kg"}`; a name already open on the list gets the amount added |
| `PATCH/DELETE /items/{id}` | Edit an item; `{"checked": true}` puts it in the cart |
| `POST /items/{id}/step` | `{"by": 1}` or `{"by": -1}`: change the amount by its step |
| `POST /items/restore` | `{"ids": [...]}`: undo a delete or *finish* |
| `POST /lists/{id}/finish` | Clear the items in the cart ("finish shopping") |
| `GET/POST /shops/{id}/staples`, `PATCH/DELETE /staples/{id}` | Staples |
| `POST /lists/{id}/staples` | `{"staples": [{"staple_id": "…"}]}`: add staples to a list |
| `GET /catalog?q=milk&shop_id=…` | Suggestions from your item catalogue (without `q`: all of it) |
| `GET /catalog/frequent?list_id=…` | What you buy most at the list's shop and isn't on the list |
| `PATCH/DELETE /catalog/{id}`, `POST /catalog/merge` | Manage the catalogue |
| `GET /sync?since=<rev>` | What changed since `rev` (0: everything): rows, `deleted`, `visible_lists` and the new `rev` |
| `GET /events` | Server-sent events: `change` (with the new rev) when something you can see changed; then call `/sync`. `POST /events/ticket` gives a one-time `?ticket=` for clients that can't send headers |
| `GET/POST /persons`, `PATCH/DELETE /persons/{id}` | People you collect gift ideas for (`user_id` links their account, hiding gift lists about them from it) |
| `GET/POST /wishlists`, `PATCH/DELETE /wishlists/{id}`, `POST /wishlists/{id}/move` | Wishlists: `{"kind": "own"}` or `{"kind": "gift", "recipient_person_id": "…"}` |
| `POST /wishlists/{id}/sections`, `PATCH/DELETE /wish-sections/{id}`, `POST /wish-sections/{id}/move` | Topic sections |
| `POST /wishlists/{id}/items`, `PATCH/DELETE /wish-items/{id}`, `POST /wish-items/{id}/move`, `POST /wish-items/restore` | Wishes: `title`, `url`, `price`, `currency`, `note`, `priority` 0–2, `section_id`; `PATCH {"received": true}` archives |
| `POST /wish-items/{id}/claim`, `PATCH/DELETE /claims/{id}` | "I'll get this", `{"status": "bought"}`, release. Never visible to the owner of an own wishlist |
| `GET/POST /wishlists/{id}/shares` | Sharing a wishlist (same as lists) |
| `GET /summary` | What needs buying on each list, with links: see [Streamline](guides/streamline.md) |
| `GET/POST /integrations`, `PATCH/DELETE /integrations/{id}`, `POST /integrations/{id}/sync`, `GET /integrations/{id}/mealie/lists` | Mealie (changes need a browser session; tokens are never returned) |
| `GET/POST /lists/{id}/shares`, `PATCH/DELETE /shares/{id}` | Sharing: `{"principal_type": "user" \| "household", "principal_id": "…", "role": "editor" \| "viewer"}`; delete your own share to leave |
| `POST /items/{id}/move` | Reorder an item: `{"after": "<item id>"}` or `{"before": …}` (neither = the end); also `category_id` and `list_id` to move it to another category or list |
| `POST /items/move` | `{"ids": [...], "list_id": "…"}`: move several items to another list |
| `POST /categories/{id}/move`, `/lists/{id}/move`, `/shops/{id}/move`, `/staples/{id}/move` | Reorder: `{"after": …}` or `{"before": …}` |
| `GET/POST /catalog/{id}/products`, `PATCH/DELETE /products/{id}` | Brands/products of a catalogue item. Items and staples take a `product_id` |
| `PUT /catalog/{id}/preferred` | `{"product_id": "…", "shop_id": "…"}` (no `shop_id`: everywhere; `null` product: no preference) |
| `POST /images` · `GET /images/{id}[?size=large]` | Upload a picture (JPEG/PNG/WebP as the body, up to `MAX_UPLOAD_MB`); it's resized and re-encoded |
| `GET /lookup?q=` · `GET /lookup/barcode/{code}` | Search Open Food Facts by name or barcode (503 when switched off or unreachable) |
| `GET /lookup/picture/{code}` · `POST /lookup/{code}/import` | A result's picture (via your server); use a result as a product or picture |

Adding milk to your supermarket's default list from a script:

```sh
curl -X POST -H "Authorization: Bearer sc_…" -H "Content-Type: application/json" \
  -d '{"text": "milk 2"}' http://cart.local:3100/api/v1/shops/<shop id>/items
```

## Coming with later phases

| Endpoint | Purpose |
|---|---|

Keeping a dashboard up to date:

```sh
rev=0
curl -s -N -H "Authorization: Bearer sc_…" http://cart.local:3100/api/v1/events |
  while read -r line; do
    case "$line" in data:*) curl -s -H "Authorization: Bearer sc_…" \
      "http://cart.local:3100/api/v1/sync?since=$rev" > latest.json
      rev=$(jq .rev latest.json);; esac
  done
```

## Conventions
- Ids are [ULIDs](https://github.com/ulid/spec). Clients may choose the id when creating
  something; repeating a create with the same id is safe.
- Errors are [problem documents](https://www.rfc-editor.org/rfc/rfc9457) with a `type`,
  `title` and `detail`.
- Something you can't see returns `404`, the same as something that doesn't exist.
