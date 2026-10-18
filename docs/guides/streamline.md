# Streamline

!!! note "Streamline side planned"
    ShoppingCart's side (the summary API) is ready; the provider in Streamline is built next,
    in the Streamline repository.

[Streamline](https://github.com/Laurits7/Streamline) can remind you to go shopping: while a
list has items, it keeps one task such as **"Go shopping: Rimi (12 items)"** with the items in
its details and a link to the list. When the list is empty, the task completes itself.

## Setting it up
1. In ShoppingCart, **Settings → API tokens** → create a token with **read** scope.
2. In Streamline, add a ShoppingCart integration with ShoppingCart's address and the token,
   and pick which lists or shops to watch and from how many items a task should appear.
3. Set `PUBLIC_URL` in ShoppingCart so the links in Streamline's tasks point to the address
   you use in the browser.

## What Streamline calls
`GET /api/v1/summary` returns, for every list you can see (not archived), its shop, how many
items are still open, the first items (with amounts and categories), when it last changed and a
link that opens it. A **read** token is enough.

```sh
curl -H "Authorization: Bearer sc_…" "http://cart.local:3100/api/v1/summary?non_empty=true"
```

```json
[{"list_id": "01J…", "list_name": "Shopping list", "shop_name": "Rimi", "owner_name": "Laurits",
  "role": "owner", "is_default": true, "open_count": 3,
  "items": [{"id": "01J…", "name": "milk", "amount": "2", "category": "Dairy and eggs"},
            {"id": "01J…", "name": "flour", "amount": "1 kg", "category": null}],
  "updated_at": "2026-10-18T17:02:11.123Z",
  "url": "https://cart.example.com/list/01J…"}]
```

Parameters: `limit` (items per list, default 20), `shop_id` or `list_id` to narrow it down, and
`non_empty=true` to leave out lists with nothing to buy. Links use `PUBLIC_URL`; without it,
the address Streamline used to call ShoppingCart.

With a **write** token, `POST /api/v1/lists/{id}/finish` clears what's in the cart, for an app
that wants to mark a shopping run as done.
