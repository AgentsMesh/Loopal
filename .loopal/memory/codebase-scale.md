---
name: Codebase Scale & Limits
description: Project size metrics, runtime limits, and complexity hotspots for planning
type: project
created_at: 2026-04-14
updated_at: 2026-04-14
ttl_days: 90
related:
  - windows-ci-gotchas.md
  - release-ci.md
---

## Scale

- ~186K LOC Rust, ~33 internal crates
- Rust 1.94.0, edition 2024, Bazel 8.1.0 (pinned in MODULE.bazel L30-32)

## Largest Crates (LOC)

1. loopal-tui — 17K
2. loopal-runtime — 12K
3. tools/filesystem — 7K
4. loopal-provider — 6K
5. loopal-agent-hub — 5K

These five crates account for ~50K LOC (~27% of total). Refactoring or large changes in these areas need extra care and may benefit from sub-agent delegation.

## Runtime Limits

- Hub supports up to **16 sub-agents** (`hub.rs` `max_total_agents: 16`)
- Optional **MetaHub** enables cross-hub clustering — sub-hubs register via uplink (`loopal-agent-hub/src/uplink.rs`), agents can spawn on remote hubs via `target_hub` parameter
