# ShoppingCart: one static binary (API + embedded web app) in an empty image.

# 1. Web app
FROM node:24-alpine AS web
WORKDIR /web
COPY web/package.json web/package-lock.json ./
RUN npm ci --no-audit --no-fund
COPY web/ ./
RUN npm run build

# 2. Server: a static musl binary with the web app embedded
FROM rust:1-alpine AS server
RUN apk add --no-cache musl-dev
# Low-memory hosts (e.g. a Raspberry Pi) can build with
# --build-arg LTO=thin --build-arg CODEGEN_UNITS=16: slightly larger binary, far less RAM.
ARG LTO=true
ARG CODEGEN_UNITS=1
ENV CARGO_PROFILE_RELEASE_LTO=$LTO CARGO_PROFILE_RELEASE_CODEGEN_UNITS=$CODEGEN_UNITS
WORKDIR /src
# Build dependencies first against stub sources, so code changes don't recompile them.
COPY Cargo.toml Cargo.lock ./
COPY crates/domain/Cargo.toml crates/domain/
COPY crates/server/Cargo.toml crates/server/
RUN mkdir -p crates/domain/src crates/server/src \
 && touch crates/domain/src/lib.rs crates/server/src/lib.rs \
 && echo 'fn main() {}' > crates/server/src/main.rs \
 && cargo build --release --locked -p shoppingcart \
 && rm -rf crates/*/src target/release/.fingerprint/shoppingcart*
COPY crates crates
COPY migrations migrations
COPY --from=web /web/dist web/dist
RUN cargo build --release --locked -p shoppingcart \
 && cp target/release/shoppingcart /shoppingcart \
 && mkdir /data && chown 10001:10001 /data

# 3. Runtime: the binary, CA certificates (for integrations later) and nothing else
FROM scratch
COPY --from=server /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
COPY --from=server /shoppingcart /shoppingcart
COPY --from=server --chown=10001:10001 /data /data
USER 10001:10001
ENV DATA_DIR=/data PORT=3100
EXPOSE 3100
VOLUME /data
HEALTHCHECK --interval=30s --timeout=5s --start-period=5s CMD ["/shoppingcart", "healthcheck"]
ENTRYPOINT ["/shoppingcart"]
