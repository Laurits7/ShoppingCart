# HTTPS and the phone app

ShoppingCart works over plain HTTP on your home network. Over **HTTPS** it also:

- can be **installed** as an app on phones and computers (PWA),
- keeps the app itself available when the connection drops in the shop (service worker),
- marks its cookies `Secure`.

Put it behind any reverse proxy and set `TRUST_PROXY: "true"` (and `PUBLIC_URL`).

To install it: in Chrome or Edge, **Settings → Install the app** (or the install icon in the
address bar); on iPhone and iPad, Safari's **Share → Add to Home Screen**. When a new version
of ShoppingCart is running, the app offers **Reload** to switch to it.

Without a connection the app also opens with your lists as they were last synced (saved on
the device, deleted when you sign out), and changes are sent when the connection is back.

=== "Caddy"

    ```caddy
    cart.example.com {
        reverse_proxy shoppingcart:3100
    }
    ```

=== "Tailscale"

    ```sh
    tailscale serve --bg --https=443 http://localhost:3100
    ```

=== "Traefik"

    ```yaml
    labels:
      - traefik.http.routers.cart.rule=Host(`cart.example.com`)
      - traefik.http.routers.cart.tls.certresolver=letsencrypt
      - traefik.http.services.cart.loadbalancer.server.port=3100
    ```

Live updates use Server-Sent Events: make sure the proxy doesn't buffer responses (Caddy and
Traefik don't by default; for nginx set `proxy_buffering off;` on `/api/v1/events`).
