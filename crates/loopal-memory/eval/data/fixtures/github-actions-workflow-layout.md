---
name: GitHub Actions Workflow Layout
description: How ci.yml, deploy.yml, and nightly.yml are split and what each owns.
type: reference
created_at: 2026-01-10
updated_at: 2026-03-30
ttl_days: null
related:
  - cicd-pipeline-overview
  - test-parallelization-shards
  - docker-multistage-build
---

ci.yml runs on every PR (lint + unit + 8-shard integration via
[[test-parallelization-shards]]), deploy.yml triggers on tag push and
feeds [[deploy-canary-rollout]], nightly.yml rebuilds base images per
[[docker-multistage-build]] at 03:17 UTC. All three reuse the matrix
defined in [[cicd-pipeline-overview]].
