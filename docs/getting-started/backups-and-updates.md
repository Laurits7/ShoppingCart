# Backups and updates

## Backups

All data is in `./data`: `shoppingcart.db` (SQLite), `secret.key` (which encrypts stored
integration tokens) and the `images/` folder with product pictures.

- **Nightly backups**: every night a consistent copy of the database is written to
  `data/backups/` and the last 7 are kept (`BACKUP_KEEP`, 0 switches this off). Admins see
  them in **Settings → Backups**, can make one now and download them.
- **Off the machine**: copy the whole `data` folder somewhere else regularly (it includes
  `backups/`, `images/` and `secret.key`). The newest file in `backups/` is safe to copy while
  ShoppingCart runs.
- **Your own data**: anyone can download what's theirs (shops, lists, templates, catalogue,
  brands, wishlists and gift lists) as JSON in **Settings → Your data**.

To restore, stop the container, put a backup in place of `data/shoppingcart.db` (remove any
`shoppingcart.db-wal` and `-shm` next to it) and start it again.

## Updates

With the prebuilt image:

```sh
docker compose pull
docker compose up -d
```

(Or put the new release's version in `image:` first.) Building from the code:
`git pull && docker compose up -d --build`.

Database migrations run automatically on start. Make a backup first (**Settings → Backups →
Back up now**). What changed is in the
[changelog](https://github.com/Laurits7/ShoppingCart/blob/main/CHANGELOG.md).
