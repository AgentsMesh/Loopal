---
name: Self-Hosted Runner Pool
description: 12-node m6i.2xlarge fleet behind actions-runner-controller on EKS.
type: reference
created_at: 2026-01-12
updated_at: 2026-04-18
ttl_days: null
related:
  - cicd-pipeline-overview
  - runner-autoscale-tuning
  - secrets-oidc-vault
---

Runners are ephemeral pods (one job per pod) managed by
actions-runner-controller v0.9.3 on the shared EKS cluster; scaling
rules live in [[runner-autoscale-tuning]]. Each pod assumes an IAM role
via OIDC as described in [[secrets-oidc-vault]] — no static PATs since
the Jan 2026 rotation incident referenced in [[cicd-pipeline-overview]].
