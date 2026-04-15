# Project Memory

This file is managed by Loopal to remember key facts about the project.

## Project

- [Codebase Scale & Limits](codebase-scale.md) — ~186K LOC / 33 crates, Rust 1.94.0; top 5 crates (tui 17K, runtime 12K, filesystem 7K, provider 6K, hub 5K) hold ~27% of code; Hub max 16 sub-agents; MetaHub for cross-hub clustering (2026-04)
- [Windows CI Gotchas](windows-ci-gotchas.md) — PATH length limit breaks rules_rust builds; shorten output_base (2026-04)
- [Release CI Pipeline](release-ci.md) — v0.1.1 released 2026-04-15; builds 4 platforms in ~17min (Windows slowest at 16m); **ACTION**: update checkout/upload-artifact/setup-bazel actions before Node.js 24 forced migration on 2026-06-02 (2026-04)
