---
name: Deploy Canary Rollout
description: 5%/25%/100% Argo Rollouts canary with SLO-gated promotion via Flagger metrics.
type: reference
created_at: 2026-02-05
updated_at: 2026-04-19
ttl_days: null
related:
  - cicd-pipeline-overview
  - secrets-oidc-vault
  - dashboard-deploy
  - blue-green-fallback
---

Argo Rollouts steps: 5% for 5min, 25% for 10min, then 100%. Promotion
gate queries Prometheus for p99 latency < 380ms and 5xx rate < 0.2%
(metrics surfaced in [[dashboard-deploy]]). If gate fails twice we fall
back to [[blue-green-fallback]] and page on-call. Creds via
[[secrets-oidc-vault]].
