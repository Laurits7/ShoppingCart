# HTTPS and the phone app

ShoppingCart works over plain HTTP on your home network. Over **HTTPS** it also:

- can be **installed** as an app on phones and computers (PWA),
- keeps the app itself available when the connection drops in the shop (service worker),
- marks its cookies `Secure`.

Put it behind any reverse proxy and set `TRUST_PROXY: "true"` (and `PUBLIC_URL`).

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
