---
name: Grafana Production Dashboard
description: Oncall-watched production metrics
type: reference
created_at: 2026-02-25
updated_at: 2026-04-20
ttl_days: null
related:
  - dashboard-deploy
  - dashboard-metrics
---

Production Grafana board: grafana.internal/d/loopal-prod. Watched by oncall during deploys. Panels: request rate, p99 latency, error budget, queue depth. Metrics schema in [[dashboard-metrics]], deploy procedure in [[dashboard-deploy]].
