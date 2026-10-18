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

if [ -d web/dist/assets ]; then
  total=0
  for f in web/dist/assets/*.js web/dist/assets/*.css; do
    total=$(( total + $(gzip -9c "$f" | wc -c) ))
  done
  echo "web:    $(( total / 1024 )) KB gzip (JS + CSS)"
fi
