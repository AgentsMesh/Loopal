---
name: CI/CD Pipeline Overview
description: Hub doc mapping our GitHub Actions topology, runner pool, and deploy stages.
type: reference
created_at: 2026-01-08
updated_at: 2026-04-22
ttl_days: null
related:
  - github-actions-workflow-layout
  - self-hosted-runner-pool
  - deploy-canary-rollout
  - artifact-cache-strategy
  - flaky-test-quarantine
  - secrets-oidc-vault
---

Our pipeline fans out from [[github-actions-workflow-layout]] into the
[[self-hosted-runner-pool]] (12 m6i.2xlarge nodes) for build+test, then
hands artifacts to [[deploy-canary-rollout]] for staged rollout. Cache,
secrets, and quarantine policy are tracked in [[artifact-cache-strategy]],
[[secrets-oidc-vault]], and [[flaky-test-quarantine]] respectively.
