---
name: SLO Definitions and Error Budgets
description: Per-service SLOs, error budget windows, and burn-rate alert thresholds
type: reference
created_at: 2026-02-08
updated_at: 2026-05-20
ttl_days: null
related:
  - observability-stack
  - alert-rules-catalog
  - incident-postmortem-template
---

Scanner: 99.5% success over 30d (3.6h budget); Dashboard: 99.9% p95<400ms over 30d. Burn-rate alerts fire at 14.4x (1h window) and 6x (6h window) per the Google SRE multi-window approach, wired through [[alert-rules-catalog]]. Every exhausted budget triggers an [[incident-postmortem-template]] within 5 business days, and SLI sources are documented next to [[dashboard-metrics]].
