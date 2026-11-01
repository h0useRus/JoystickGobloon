#!/usr/bin/env bash
# Clones a Pages repository of h0useRus into ./out with the deploy key in $DEPLOY_KEY, runs a
# command that fills it, and pushes the result as one commit (nothing when unchanged).
# Usage: push-pages.sh <repository> <fill command> <commit message>
set -euo pipefail
repo=$1
fill=$2
message=$3
key="$RUNNER_TEMP/${repo}_deploy"
printf '%s\n' "$DEPLOY_KEY" > "$key"
chmod 600 "$key"
export GIT_SSH_COMMAND="ssh -i $key -o IdentitiesOnly=yes -o StrictHostKeyChecking=accept-new"
rm -rf out
git clone --depth 1 "git@github.com:h0useRus/$repo.git" out
eval "$fill"
cd out
git config user.name "github-actions[bot]"
git config user.email "41898282+github-actions[bot]@users.noreply.github.com"
git add -A
if git diff --cached --quiet; then
  echo "No changes."
  exit 0
fi
git commit -m "$message"
git push
