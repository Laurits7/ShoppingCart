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

## Coming with later phases

| Endpoint | Purpose |
|---|---|
| `GET /health` | Liveness (no auth) |
| `GET /sync?since=<rev>` | Everything you can see (`since=0`) or changed since `rev`, including deletions |
| `GET /events` | Server-sent events with live changes |
| `GET/POST/PATCH/DELETE /shops[/{id}]` | Shops |
| `POST /shops/{id}/items` | Add an item to the shop's **default list** (handy for shortcuts and Home Assistant) |
| `GET/POST/PATCH/DELETE /lists[/{id}]` | Lists |
| `POST /lists/{id}/finish` | Clear the ticked items ("finish shopping") |
| `GET/POST/PATCH/DELETE /lists/{id}/categories`, `/categories/{id}` | Categories |
| `GET/POST/PATCH/DELETE /lists/{id}/items`, `/items/{id}` | Items. `PATCH {"checked": true}` ticks an item. Adding a name already open on the list adds to its amount |
| `POST /items/{id}/step` | `{"by": 1}` or `{"by": -1}`: change the amount by its step |
| `GET/POST/PATCH/DELETE /catalog/{id}/products`, `/products/{id}` | Brands/products of a catalogue item; `PUT /catalog/{id}/preferred` |
| `POST /images` · `GET /images/{id}` | Upload a picture (resized on the server) and fetch it |
| `GET /lookup?q=` · `GET /lookup/barcode/{code}` | Search Open Food Facts for products and pictures |
| `GET /catalog?q=milk` | Suggestions from your item catalogue; `GET/PATCH/DELETE /catalog/{id}`, `POST /catalog/merge` to manage it |
| `POST /items/{id}/move` | Move/reorder (`list_id`, `category_id`, `after`/`before`) |
| `GET/POST/DELETE /lists/{id}/shares` | Sharing |
| `GET /summary` | Open items per list, for Streamline and dashboards |
| `/wishlists`, `/wish-items`, `/claims`, `/persons` | Wishlists and gifts |
| `/me`, `/tokens`, `/sessions`, `/users`, `/households` | Account and administration |

## Conventions
- Ids are [ULIDs](https://github.com/ulid/spec). Clients may choose the id when creating
  something; repeating a create with the same id is safe.
- Errors are [problem documents](https://www.rfc-editor.org/rfc/rfc9457) with a `type`,
  `title` and `detail`.
- Something you can't see returns `404`, the same as something that doesn't exist.
