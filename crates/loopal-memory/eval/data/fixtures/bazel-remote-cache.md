---
name: Bazel Remote Cache
description: buchgr/bazel-remote on s3://loopal-bazel-cache, 500GB LRU, 78% hit rate on main.
type: reference
created_at: 2026-02-02
updated_at: 2026-04-15
ttl_days: null
related:
  - artifact-cache-strategy
  - test-parallelization-shards
---

We run buchgr/bazel-remote v2.4.5 behind an internal ALB, backed by
s3://loopal-bazel-cache with 500GB LRU eviction. Hit rate on main is
~78% (measured via `bazel-remote-stats`); PR branches drop to ~42% due
to Cargo.lock churn — same root cause as [[artifact-cache-strategy]].
Shard configuration in [[test-parallelization-shards]] depends on this
being warm.
