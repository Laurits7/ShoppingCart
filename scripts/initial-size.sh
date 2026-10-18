#!/bin/sh
# Size of the initial load (SPEC §4.1): the JS and CSS that dist/index.html references (entry
# script, module preloads, stylesheet), gzipped one by one as a browser receives them. Chunks
# loaded on demand later don't count. Fails above the budget. Run from web/.
set -eu
budget=${BUDGET_BYTES:-61440}
files=$(grep -oE '/assets/[^"]+\.(js|css)' dist/index.html | sort -u)
total=0
for f in $files; do
  total=$(( total + $(gzip -9c "dist$f" | wc -c) ))
done
lazy=$(ls dist/assets/*.js | wc -l)
echo "$(( total / 1024 )) KB gzip initial JS + CSS ($(echo "$files" | wc -l) files; $lazy JS files in all)"
test "$total" -lt "$budget"
