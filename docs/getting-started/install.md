# Install

ShoppingCart runs as one Docker container. You need Docker with the Compose plugin.

1. Either download the `docker-compose.yml` attached to the latest
   [release](https://github.com/Laurits7/ShoppingCart/releases) into a new folder and run
   `mkdir data` there (it uses the prebuilt image for amd64 and arm64), or get the code to
   build it yourself:

    ```sh
    git clone https://github.com/Laurits7/ShoppingCart.git
    cd ShoppingCart
    ```

2. If your user id isn't 1000 (check with `id -u`), set `PUID` and `PGID` before starting,
   e.g. `export PUID=$(id -u) PGID=$(id -g)`. The container runs as you, not as root, and
   writes only to `./data`.

3. Start it:

    ```sh
    docker compose up -d
    ```

    With the prebuilt image this only downloads it. Building from the code takes a few
    minutes; on a low-memory device such as a Raspberry Pi, see the build arguments in
    `docker-compose.yml`.

4. Open `http://<server-ip>:3100`. The first visit asks you to create the **admin** account.
   (Or set `INITIAL_ADMIN_USER` and `INITIAL_ADMIN_PASSWORD`, see
   [Configuration](configuration.md).)

5. As admin, add the other people in **Settings → Users**, and create a household in
   **Settings → Households** to share lists with them (anyone can create a household).

!!! tip "Only downloaded the compose file?"
    Create the data folder yourself first (`mkdir data`), so it belongs to you. If the container
    stops with *can't write to the data folder*, fix the owner with
    `sudo chown -R $(id -u):$(id -g) data`.

Your data is in `./data` next to `docker-compose.yml`: the database and the secret key. Back
that folder up (see [Backups and updates](backups-and-updates.md)).

To use ShoppingCart as an app on your phone, serve it over HTTPS
([HTTPS and the phone app](https.md)).
