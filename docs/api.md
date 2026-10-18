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
| `POST /items/{id}/move` | Reorder an item: `{"after": "<item id>"}` or `{"before": …}` (neither = the end); also `category_id` and `list_id` to move it to another category or list |
| `POST /items/move` | `{"ids": [...], "list_id": "…"}`: move several items to another list |
| `POST /categories/{id}/move`, `/lists/{id}/move`, `/shops/{id}/move`, `/staples/{id}/move` | Reorder: `{"after": …}` or `{"before": …}` |

Adding milk to your supermarket's default list from a script:

```sh
curl -X POST -H "Authorization: Bearer sc_…" -H "Content-Type: application/json" \
  -d '{"text": "milk 2"}' http://cart.local:3100/api/v1/shops/<shop id>/items
```

## Coming with later phases

| Endpoint | Purpose |
|---|---|
| `GET /sync?since=<rev>` | Everything you can see (`since=0`) or changed since `rev`, including deletions |
| `GET /events` | Server-sent events with live changes |
| `GET/POST/PATCH/DELETE /catalog/{id}/products`, `/products/{id}` | Brands/products of a catalogue item; `PUT /catalog/{id}/preferred` |
| `POST /images` · `GET /images/{id}` | Upload a picture (resized on the server) and fetch it |
| `GET /lookup?q=` · `GET /lookup/barcode/{code}` | Search Open Food Facts for products and pictures |
| `GET/POST/DELETE /lists/{id}/shares` | Sharing |
| `GET /summary` | Open items per list, for Streamline and dashboards |
| `/wishlists`, `/wish-items`, `/claims`, `/persons` | Wishlists and gifts |

## Conventions
- Ids are [ULIDs](https://github.com/ulid/spec). Clients may choose the id when creating
  something; repeating a create with the same id is safe.
- Errors are [problem documents](https://www.rfc-editor.org/rfc/rfc9457) with a `type`,
  `title` and `detail`.
- Something you can't see returns `404`, the same as something that doesn't exist.
