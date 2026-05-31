---
name: Artifact Cache Strategy
description: actions/cache + sccache + BuildKit registry cache, keyed on Cargo.lock + rustc hash.
type: reference
created_at: 2026-01-20
updated_at: 2026-04-05
ttl_days: null
related:
  - docker-multistage-build
  - cicd-pipeline-overview
  - bazel-remote-cache
---

Key is `${{ hashFiles('Cargo.lock') }}-${{ env.RUSTC_HASH }}`; partial
restore via `restore-keys` falls back to last green main. BuildKit
registry cache (mode=max) sits at ghcr.io/loopal/buildcache and is
pruned weekly. Bazel builds use a separate [[bazel-remote-cache]];
overview in [[cicd-pipeline-overview]] and consumed by
[[docker-multistage-build]].
