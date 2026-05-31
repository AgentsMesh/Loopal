---
name: Autovacuum Tuning for Hot Tables
description: Per-table autovacuum_vacuum_scale_factor overrides for high-churn tables
type: reference
created_at: 2026-02-14
updated_at: 2026-04-25
ttl_days: null
related:
  - postgres-runbook
  - slow-query-triage
  - index-bloat-investigation
  - partition-rotation-cron
---

Global autovacuum_vacuum_scale_factor=0.2 is too lazy for events / audit_log. Override to 0.02 + autovacuum_vacuum_cost_limit=2000. Tables partitioned by month (see [[partition-rotation-cron]]) inherit per-partition settings. See [[index-bloat-investigation]] for when REINDEX CONCURRENTLY is needed instead of vacuum. Triage path: [[slow-query-triage]].
