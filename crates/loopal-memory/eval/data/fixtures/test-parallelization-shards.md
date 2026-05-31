---
name: Test Parallelization Shards
description: 8-shard integration matrix using nextest partition; ~7min p95 vs 38min serial.
type: reference
created_at: 2026-01-25
updated_at: 2026-03-22
ttl_days: null
related:
  - github-actions-workflow-layout
  - bazel-remote-cache
  - flaky-test-quarantine
---

We partition with `cargo nextest run --partition hash:${SHARD}/8` per
[[github-actions-workflow-layout]]. Each shard hits warm
[[bazel-remote-cache]] (~78% hit) and skips quarantined tests from
[[flaky-test-quarantine]]. p95 wall time dropped from 38min serial to
7min sharded; tail is shard 3 (network-heavy integration suite).
