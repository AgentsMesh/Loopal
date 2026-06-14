---
name: Release CI Pipeline
description: Release pipeline (4 targets), version history (current v0.6.3), and OVERDUE Node.js 24 action upgrade (deadline 2026-06-02 passed)
type: project
created_at: 2026-04-15
updated_at: 2026-06-14
ttl_days: 90
related:
  - windows-ci-gotchas.md
  - codebase-scale.md
---

## Current Version

v0.6.3 (tag dated 2026-06-12)

Version lineage: v0.0.1-alpha → v0.0.1-alpha.1 → v0.0.1-alpha.2 → v0.1.0 → v0.1.1 → v0.2.0 → v0.3.0 → v0.4.0 → v0.5.0 → v0.6.0 → v0.6.1 → v0.6.2 → v0.6.3

## Release Pipeline

Builds 4 platform targets in `/Users/stone/Works/Loopal/.github/workflows/release.yml` (triggered on `v*` tag push):
- macOS ARM64 (`aarch64-apple-darwin`, runner macos-14)
- Linux x86_64 (`x86_64-unknown-linux-gnu`, runner ubuntu-latest)
- Linux ARM64 (`aarch64-unknown-linux-gnu`, runner ubuntu-24.04-arm)
- Windows x86_64 (`x86_64-pc-windows-msvc`, runner windows-latest)

Build via `bazel build //:loopal -c opt`; a separate `release` job collects artifacts and publishes with `softprops/action-gh-release@v2`. Windows is historically the bottleneck — re-check timings after Windows/rules_rust changes (see `windows-ci-gotchas.md`).

## OVERDUE — Node.js 24 action upgrade (deadline was 2026-06-02)

GitHub Actions force-upgraded Node.js 20 → 24 on **2026-06-02** (deadline has passed). As of 2026-06-14, both `.github/workflows/release.yml` (last modified 2026-04-11) and `ci.yml` still pin the pre-deadline versions:

- `actions/checkout@v4`
- `actions/upload-artifact@v4` / `actions/download-artifact@v4` (release.yml)
- `bazel-contrib/setup-bazel@0.14.0`

These were not bumped before the deadline. Verify they are Node.js 24-compatible or upgrade to current major versions; what was a non-blocking warning may now be failing.
