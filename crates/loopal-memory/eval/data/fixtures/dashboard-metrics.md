---
name: Dashboard Metrics Schema
description: What the dashboard exposes and units
type: project
created_at: 2026-02-22
updated_at: 2026-04-25
ttl_days: 90
related:
  - dashboard-deploy
  - reference-grafana-dashboard
---

Exposed metrics: request_count (counter), p50/p95/p99 latency (histogram, ms), error_rate (gauge). All are prefixed `loopal_` and scraped at 15s intervals. Deployment process in [[dashboard-deploy]] depends on these endpoints being live during smoke checks. Production view at [[reference-grafana-dashboard]].
