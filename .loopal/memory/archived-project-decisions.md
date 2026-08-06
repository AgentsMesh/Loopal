---
name: Archived Project Decisions
description: Retain the Windows rules_rust PATH workaround until its upstream fix is verified; releases target four platforms and require Node.js-compatible actions; the hub defaults to 16 agents with MetaHub remote spawning
related: []
type: reference
created_at: 2026-08-04
updated_at: 2026-08-04
ttl_days: null
---

- Release tags build macOS ARM64, Linux x86_64, Linux ARM64, and Windows x86_64 artifacts; keep GitHub Actions dependencies compatible with the active Node.js runtime.
- Retain `patches/rules_rust_windows_consolidate_deps.patch` while `MODULE.bazel` requires the rules_rust Windows PATH-length workaround; remove it only after verifying the upstream fix.
- The agent hub defaults to 16 total agents and supports remote spawning through MetaHub uplinks.
