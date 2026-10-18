# Configuration

Everything is optional. Set variables in the `environment:` section of `docker-compose.yml`.

| Variable | Default | Purpose |
|---|---|---|
| `PORT` | `3100` | Port inside the container. Change the published port in `ports:` instead if you only want another port on the host. |
| `DATA_DIR` | `/data` | Where the database, `secret.key` and backups live. |
| `LOG_LEVEL` | `info` | Log detail: `error`, `warn`, `info`, `debug`. |
| `PUBLIC_URL` | unset | The address people use, e.g. `https://cart.example.com`. Used in links handed to other apps (Streamline). |
| `TRUST_PROXY` | `false` | Set to `true` behind a reverse proxy, so the real client IP and HTTPS are detected. Leave it off otherwise. |
| `INITIAL_ADMIN_USER`, `INITIAL_ADMIN_PASSWORD` | unset | Create the first admin on start, instead of in the browser. Only used while no user exists. |
| `SECRET_KEY` | `data/secret.key` | Key that encrypts stored integration tokens (Mealie). Generated on first start; keep it with your backups. |
| `SESSION_DAYS` | `30` | How long you stay signed in without using the app. |
| `COOKIE_SECURE` | `auto` | `auto` marks the sign-in cookie `Secure` when a trusted proxy says the request came over HTTPS; `true`/`false` force it. |
| `BIND` | `0.0.0.0` | Address to listen on inside the container. |
| `PUID`, `PGID` | `1000` | (Compose only) The user and group the container runs as; should own `./data`. |
| `PRODUCT_LOOKUP` | `openfoodfacts` | Where product pictures and brands are looked up. `off` disables it; nothing then leaves your server. |
| `MAX_UPLOAD_MB` | `8` | Largest picture you can upload. |

The container runs as `PUID:PGID` (not root), with a read-only filesystem, no Linux
capabilities and `no-new-privileges`; only `./data` is writable.
