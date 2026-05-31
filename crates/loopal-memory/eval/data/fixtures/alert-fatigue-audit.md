---
name: Quarterly Alert Fatigue Audit
description: Process for retiring noisy alerts based on PagerDuty + ack data
type: project
created_at: 2026-03-05
updated_at: 2026-05-25
ttl_days: 180
related:
  - alert-rules-catalog
  - oncall-runbook
  - metric-cardinality-budget
---

Query PagerDuty for last-90d alerts, compute (acks_with_no_action / total_fires) per rule; >40% goes on the retire list. Q1 2026 we killed 7 rules, mostly disk-percent thresholds replaced with predict_linear-based ones documented in [[alert-rules-catalog]]. Oncall lead reviews list with [[oncall-runbook]] owner before deletion.
