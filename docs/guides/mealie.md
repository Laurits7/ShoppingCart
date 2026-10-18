# Mealie

ShoppingCart can treat a [Mealie](https://mealie.io) shopping list as an **inbox**: whatever
you add there (for example the ingredients of a recipe or the week's meal plan) moves into one
of your ShoppingCart lists.

Works with Mealie 2 and newer (written against Mealie 3.29).

## Setting it up
1. In Mealie, create an API token: your profile → **API Tokens**.
2. In ShoppingCart, open **Settings → Mealie**, enter Mealie's address, e.g.
   `http://mealie:9000` (if both run in the same compose network) or
   `https://mealie.example.com`, and the token, and **Connect**.
3. Then choose which Mealie shopping list to watch and where items should go: a shop (its
   default list) or a specific list, and **Save**. It switches itself on.
4. Optionally let Mealie's **labels** pick the category of the same name, and create it if the
   list doesn't have it yet. Choose what happens in Mealie afterwards: tick the items (the
   default), remove them, or leave them.

Mealie on your own network is fine: ShoppingCart connects to it from the server.

## What happens
Every 5 minutes (or on **Sync now**), unchecked items on the watched Mealie list are added to
your list and then ticked in Mealie. Mealie's food, quantity and unit become the item's name,
amount and unit ("500 g flour"); free-text items are read like the add box ("2 cans
tomatoes"). Each Mealie item is remembered, so it's never imported twice, even if ticking it
in Mealie failed once. Imports stay generic, unless the text is exactly one of your products'
names (see [Brands and pictures](products.md)).

If Mealie can't be reached or refuses the token, nothing else is affected: the settings page
shows the problem and when it last worked, and it tries again later (less often while it keeps
failing).

The token is stored encrypted on the server (with `SECRET_KEY` or `data/secret.key`; keep it
with your backups) and never sent back to the browser. Changing the integration needs a
signed-in browser, not an API token.

## Trying it locally
`docker compose --profile mealie-dev up -d mealie` starts a Mealie for testing at
<http://localhost:9925>, at the version the integration was written for.
