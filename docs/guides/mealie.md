# Mealie

!!! note "Planned"
    This integration is Phase 6 of the [work plan](../WORKPLAN.md).

ShoppingCart can treat a [Mealie](https://mealie.io) shopping list as an **inbox**: whatever
you add there (for example the ingredients of a recipe or the week's meal plan) moves into one
of your ShoppingCart lists.

## Setting it up
1. In Mealie, create an API token: your profile → **API Tokens**.
2. In ShoppingCart, open **Settings → Integrations → Mealie** and enter:
    - Mealie's address, e.g. `http://mealie:9000` (if both run in the same compose network) or
      `https://mealie.example.com`,
    - the token,
    - which Mealie shopping list to watch,
    - where items should go: a shop (its default list) or a specific list.
3. Optionally let Mealie's **labels** become categories of the same name.

## What happens
Every 5 minutes (or on *Sync now*), unchecked items on the watched Mealie list are added to
your list and then ticked in Mealie, so they aren't imported twice. Items imported from Mealie
show a small Mealie badge. If Mealie can't be reached, nothing else is affected; the
settings page shows the last error.

The token is stored encrypted on the server and never sent back to the browser.
