# Streamline

!!! note "Planned"
    The API side is Phase 6 of the [work plan](../WORKPLAN.md); the Streamline side is built
    afterwards in the Streamline repository.

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
`GET /api/v1/summary` returns, for every list you can see, its shop, open item count, the
first items and a link. See [API](../api.md).
