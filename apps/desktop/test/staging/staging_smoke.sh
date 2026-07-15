#!/usr/bin/env bash
set -euo pipefail

stage="$1"
test -f "$stage/package.json"
test -f "$stage/out/main/index.cjs"
test -f "$stage/out/preload/index.cjs"
test -f "$stage/out/renderer/index.html"
test -f "$stage/builder-after-extract.cjs"
test -f "$stage/builder-before-build.cjs"
test -x "$stage/runtime/loopal"
grep -q '"main":"./out/main/index.cjs"' "$stage/package.json"
grep -q 'beforeBuild: ./dist_staging/builder-before-build.cjs' "$stage/electron-builder.yml"
grep -q 'afterExtract: ./dist_staging/builder-after-extract.cjs' "$stage/electron-builder.yml"
grep -q 'electronDist: ../../../node_modules/electron/dist' "$stage/electron-builder.yml"
"$stage/runtime/loopal" --version >/dev/null
