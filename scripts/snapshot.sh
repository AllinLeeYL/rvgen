#!/usr/bin/env bash
set -euo pipefail

tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT

index="$tmpdir/index"

# Start from current HEAD using a temporary Git index
GIT_INDEX_FILE="$index" git read-tree HEAD

# Capture current working tree, including uncommitted and untracked
# non-ignored files
GIT_INDEX_FILE="$index" git add -A

tree="$(GIT_INDEX_FILE="$index" git write-tree)"

# Intentionally no -p: create an orphan/root commit
commit="$(
  printf 'Snapshot %s\n' "$(date -Iseconds)" |
    git commit-tree "$tree"
)"

git push --force origin \
  "$commit:refs/heads/snapshot"

echo "Snapshot: $commit"