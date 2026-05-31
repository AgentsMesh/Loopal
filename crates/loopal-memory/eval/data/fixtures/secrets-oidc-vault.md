---
name: Secrets via OIDC + Vault
description: GitHub OIDC -> HashiCorp Vault JWT auth, short-lived AWS STS tokens, no PATs.
type: reference
created_at: 2026-01-22
updated_at: 2026-04-01
ttl_days: null
related:
  - self-hosted-runner-pool
  - deploy-canary-rollout
  - cicd-pipeline-overview
---

Workflows exchange the GitHub OIDC token for a Vault JWT (role
`gha-loopal-main`), which mints 15-minute AWS STS creds. Documented in
[[cicd-pipeline-overview]] and consumed by both
[[self-hosted-runner-pool]] pods and [[deploy-canary-rollout]] gates.
The Jan 2026 PAT leak (gist accidentally public) forced this migration.
