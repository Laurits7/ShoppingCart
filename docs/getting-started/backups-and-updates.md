# Backups and updates

## Backups

All data is in `./data`: `shoppingcart.db` (SQLite), `secret.key` and the `images/` folder
with product pictures. To back up, either:

- stop the container and copy the folder, or
- copy it while running with SQLite's own tool:
  `sqlite3 data/shoppingcart.db ".backup 'backup.db'"`, plus `secret.key` and `images/`.

!!! note "Planned"
    Automatic nightly backups in `data/backups/` and a download button for admins come in
    Phase 7 of the work plan.

## Updates

```sh
git pull            # or download the new release's docker-compose.yml
docker compose up -d --build
```

Database migrations run automatically on start. Take a backup first.
