#!/usr/bin/env bash
# Bazel workspace_status_command — emits STABLE_LOOPAL_VERSION derived from git.
#
# Format:
#   - Exact tag, clean tree:           v0.4.0
#   - Tag + commits ahead, clean:      v0.4.0-3-g85b8b97-dev
#   - Dirty tree (any state):          v0.4.0-3-g85b8b97-dirty-dev
#   - No tags / non-SemVer tag:        0.0.0-dev.g85b8b97
#   - No git:                          0.0.0-unknown

set -eu

if ! command -v git >/dev/null 2>&1 || ! git rev-parse --git-dir >/dev/null 2>&1; then
    echo "STABLE_LOOPAL_VERSION 0.0.0-unknown"
    exit 0
fi

DESCRIBED=$(git describe --tags --always --dirty 2>/dev/null || echo "0.0.0-unknown")
EXACT=$(git describe --tags --exact-match HEAD 2>/dev/null || true)
DIRTY=$(git status --porcelain 2>/dev/null | head -c 1 || true)

if [ -n "${EXACT:-}" ] && [ -z "${DIRTY:-}" ]; then
    VERSION="${EXACT}"
else
    VERSION="${DESCRIBED}-dev"
fi

VERSION="${VERSION#v}"

if ! printf '%s\n' "${VERSION}" | grep -Eq \
    '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$'; then
    REVISION=$(git rev-parse --short=12 HEAD 2>/dev/null || echo unknown)
    VERSION="0.0.0-dev.g${REVISION}"
    [ -z "${DIRTY:-}" ] || VERSION="${VERSION}.dirty"
fi

echo "STABLE_LOOPAL_VERSION ${VERSION}"
