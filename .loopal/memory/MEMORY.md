# Project Memory

This file is managed by Loopal to remember key facts about the project.

## User

## Feedback

## Project

- [Codebase Scale & Limits](codebase-scale.md) — ~186K LOC / 33 crates, Rust 1.94.0; top hotspots are `loopal-tui`, `loopal-runtime`, `tools/filesystem`, `loopal-provider`, and `loopal-agent-hub`; Hub max is 16 sub-agents and MetaHub support lives in `crates/loopal-agent-hub/src/uplink.rs`. (2026-04)
- [Windows CI Gotchas](windows-ci-gotchas.md) — Windows PATH length can break rules_rust builds; this repo patches rules_rust in `/Users/stone/Works/Loopal/MODULE.bazel` to consolidate dependency paths, replacing the earlier short-`output_base` workaround. (2026-05)
- [Release CI Pipeline](release-ci.md) — latest tag is v0.4.0 (2026-05-08); release builds 4 platforms; **ACTION**: update checkout/upload-artifact/setup-bazel actions before Node.js 24 forced migration on 2026-06-02. (2026-05)

## Reference
