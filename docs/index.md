# ShoppingCart

A lean, self-hosted **shopping list and wishlist** app for a household. One small container,
one data folder, and it works from any phone, tablet or computer on your network.

!!! info "Status"
    ShoppingCart is in development. Sign-in, users, households and API tokens work today
    (Phase 1 of the [work plan](WORKPLAN.md)). The other pages describe how it works once each
    phase has landed; pages for features that don't exist yet say so.

- **Shopping lists per shop**: categories, drag-and-drop ordering, a default list and a
  staples template for every shop. See [Shops and lists](guides/shopping.md).
- **Wishlists and gift lists**: topic sections for your own wishes, and gift ideas for others
  that the recipient never sees. See [Wishlists and gifts](guides/wishlists.md).
- **Private by default**: share a list with a household or with individual people. See
  [Households and sharing](guides/sharing.md).
- **Integrations**: pull [Mealie](guides/mealie.md)'s shopping list in, let
  [Streamline](guides/streamline.md) remind you to go shopping, or use the [API](api.md).

## Quick start

```sh
docker compose up -d
```

Open `http://<server-ip>:3100` and create the admin account. Your data is in `./data`.
More in [Install](getting-started/install.md).
