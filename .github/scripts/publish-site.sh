#!/usr/bin/env bash
# Copies the website (site/) into a checkout of the Pages repository and stamps the stylesheet and
# script links with the commit, so browsers never mix a new page with a cached old styles.css.
# Usage: publish-site.sh <pages checkout> [commit]
set -euo pipefail
out=$1
rev=${2:-$(git rev-parse --short HEAD)}
find "$out" -mindepth 1 -maxdepth 1 ! -name .git -exec rm -rf {} +
cp -r site/. "$out/"
for page in "$out"/*.html; do
  sed -i -E "s#(href|src)=\"(/gobloon/)?(styles\.css|main\.js|motion\.js)\"#\1=\"\2\3?v=$rev\"#g" "$page"
done
