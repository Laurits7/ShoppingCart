# ShoppingCart

A lean, self-hosted shopping list and wishlist app for a household.

- **Shopping lists per shop**, split into categories (Dairy, Meat, Vegetables…), ordered by
  drag and drop, with a default list and a staples template per shop.
- **Wishlists** with topic sections, and **gift lists** for other people that the recipient
  never sees, with "I'll get this" claims.
- **Private by default**: share a list with your household or invite individual people.
- **Live** across devices and people, usable on a flaky connection in the shop.
- **API** (OpenAPI, tokens) with integrations for [Mealie](https://mealie.io) and
  [Streamline](https://github.com/Laurits7/Streamline).
- **Small**: one Rust binary with an embedded web app and one SQLite file.

> **Status:** in development: sign-in, users, households and API tokens work; shopping lists come
> next. See the [spec](docs/SPEC.md) and [work plan](docs/WORKPLAN.md).

## Quick start

```sh
docker compose up -d
```

Open `http://<server-ip>:3100` and create the admin account. Your data is in `./data`.

## Documentation

The docs live in [`docs/`](docs/index.md) (MkDocs). To preview them:

```sh
python3 -m venv .venv && .venv/bin/pip install -r docs/requirements.txt
.venv/bin/mkdocs serve
```

## License

[GPL-3.0](LICENSE)
