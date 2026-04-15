---
name: Release CI Pipeline
description: Release build pipeline details, version history, and upcoming CI maintenance deadlines
type: project
created_at: 2026-04-15
updated_at: 2026-04-15
ttl_days: 90
related:
  - windows-ci-gotchas.md
---

## Current Version

v0.1.1 (released 2026-04-15)

Version lineage: v0.0.1-alpha → v0.0.1-alpha.1 → v0.0.1-alpha.2 → v0.1.0 → v0.1.1

## Release Pipeline

Builds 4 platform targets:
- macOS ARM64
- Linux ARM64
- Linux x86_64
- Windows x86_64

Total wall-clock time: ~17 minutes. Windows is the bottleneck at ~16m15s (see `windows-ci-gotchas.md` for known Windows build issues).

## ACTION NEEDED by 2026-06-02

GitHub Actions will force-upgrade Node.js 20 → Node.js 24 on **2026-06-02**. The following actions must be updated to Node.js 24-compatible versions before that date:

- `actions/checkout@v4`
- `actions/upload-artifact@v4`
- `bazel-contrib/setup-bazel@0.14.0`

Currently a non-blocking warning in CI logs; will become a breaking change on the deadline.
