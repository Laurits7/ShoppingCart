#!/bin/sh
# Resource measurements for the phase gate (SPEC §4.1): image size, idle memory, bundle size.
set -eu
image=${IMAGE:-shoppingcart:latest}
container=${CONTAINER:-shoppingcart}

size=$(docker image inspect "$image" --format '{{.Size}}')
echo "image:  $(( size / 1024 / 1024 )) MB ($image)"

if docker ps --format '{{.Names}}' | grep -qx "$container"; then
  echo "memory: $(docker stats --no-stream --format '{{.MemUsage}}' "$container")"
else
  echo "memory: (container $container not running)"
fi

if [ -f web/dist/index.html ]; then
  (cd web && ../scripts/initial-size.sh) | sed 's/^/web:    /'
fi
