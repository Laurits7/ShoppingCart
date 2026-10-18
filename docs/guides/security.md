# Security

- **Login required** for everything. Users are created by the admin; there is no sign-up.
- Passwords are hashed with **argon2id**. Repeated failed logins are slowed down.
- Sessions use an `HttpOnly`, `SameSite` cookie (also `Secure` over HTTPS); only a hash of the
  session token is stored. **Settings → Sessions** shows where you're signed in.
- **API tokens** are shown once, stored hashed, can be read-only and can be revoked. Changing
  your password signs out other sessions and revokes your tokens. A token can never change
  your password, sessions or tokens: that needs a signed-in browser.
- Everything is **private by default**; every request is checked against sharing rules on
  the server, and live updates only go to people allowed to see them. Gift lists never reach
  the person they're for.
- Strict security headers (Content-Security-Policy, no framing) and cross-site request checks.
- Integration tokens (Mealie) are encrypted at rest with `secret.key` and never sent to the
  browser.
- Uploaded pictures are checked, resized and re-encoded (which removes location data from
  phone photos). Product lookups are made by the server and only send the search text or
  barcode; set `PRODUCT_LOOKUP=off` to keep everything on your network.
- The container runs as a non-root user from an otherwise empty image (just the program and CA
  certificates) with a read-only filesystem and no Linux capabilities.

Recommendations: serve it over HTTPS (see [HTTPS](../getting-started/https.md)) if it's
reachable from outside your home network, keep `./data` (which contains `secret.key`) private,
and update regularly.

Integrations may connect to addresses on your local network on purpose (a self-hosted Mealie);
only users you created can configure them.
