#!/usr/bin/env bash
set -euo pipefail

STAMP=$(cd "$(dirname "$1")" && pwd)/$(basename "$1")
REPO=$(mktemp -d "${TEST_TMPDIR:-/tmp}/loopal-version.XXXXXX")
trap 'rm -rf "$REPO"' EXIT

git -C "$REPO" init -q
git -C "$REPO" config user.email test@loopal.local
git -C "$REPO" config user.name "Loopal Test"
printf 'fixture\n' > "$REPO/README.md"
git -C "$REPO" add README.md
git -C "$REPO" -c commit.gpgsign=false commit -qm initial

revision=$(git -C "$REPO" rev-parse --short=12 HEAD)
untagged=$(cd "$REPO" && "$STAMP")
test "$untagged" = "STABLE_LOOPAL_VERSION 0.0.0-dev.g${revision}"

git -C "$REPO" tag v1.2.3
tagged=$(cd "$REPO" && "$STAMP")
test "$tagged" = "STABLE_LOOPAL_VERSION 1.2.3"

printf 'dirty\n' >> "$REPO/README.md"
dirty=$(cd "$REPO" && "$STAMP")
test "$dirty" = "STABLE_LOOPAL_VERSION 1.2.3-dirty-dev"

git -C "$REPO" checkout -q -- README.md
printf 'next\n' >> "$REPO/README.md"
git -C "$REPO" add README.md
git -C "$REPO" -c commit.gpgsign=false commit -qm next
git -C "$REPO" tag desktop-ci
revision=$(git -C "$REPO" rev-parse --short=12 HEAD)
invalid_tag=$(cd "$REPO" && "$STAMP")
test "$invalid_tag" = "STABLE_LOOPAL_VERSION 0.0.0-dev.g${revision}"

printf '%s\n' "$untagged" "$tagged" "$dirty" "$invalid_tag" | grep -Ev \
    '^STABLE_LOOPAL_VERSION (0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$' \
    && exit 1
exit 0
