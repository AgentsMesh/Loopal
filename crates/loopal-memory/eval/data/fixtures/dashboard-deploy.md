---
name: Dashboard Deploy Pipeline
description: How the analytics dashboard ships to prod
type: project
created_at: 2026-02-20
updated_at: 2026-05-03
ttl_days: 90
related:
  - dashboard-metrics
  - reference-grafana-dashboard
---

Dashboard deploys go through staging → canary (5% traffic) → full. Each step runs the smoke suite which checks the metrics endpoint described in [[dashboard-metrics]]. Production graphs live at [[reference-grafana-dashboard]] for oncall visibility.
