#!/usr/bin/env bash
# Fills a checkout of the old Pages repository (h0useRus/gobloon) with pages that send visitors to
# the website's new address: one page per page of site/ (keeping the #part of the link), and a 404
# page that forwards any other path, so old links and bookmarks keep working.
# Usage: redirect-site.sh <pages checkout> <new base URL, ending in />
set -euo pipefail
out=$1
base=$2
find "$out" -mindepth 1 -maxdepth 1 ! -name .git -exec rm -rf {} +
touch "$out/.nojekyll"

# $1 = file to write, $2 = target URL, $3 = script that sends the visitor on.
write() {
  cat > "$1" <<EOF
<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>Joystick Gobloon has moved</title>
  <meta name="robots" content="noindex">
  <link rel="canonical" href="$2">
  <meta http-equiv="refresh" content="0; url=$2">
  <script>$3</script>
</head>
<body>
  <p>The Joystick Gobloon website has moved to <a href="$2">$2</a>.</p>
</body>
</html>
EOF
}

for page in site/*.html; do
  name=$(basename "$page")
  [ "$name" = 404.html ] && continue
  url=$base$name
  [ "$name" = index.html ] && url=$base
  write "$out/$name" "$url" "location.replace('$url' + location.search + location.hash);"
done
# Any other old path (images, a page that no longer exists): the same path at the new address.
write "$out/404.html" "$base" \
  "location.replace('$base' + location.pathname.replace(/^\/gobloon\/?/, '') + location.search + location.hash);"
