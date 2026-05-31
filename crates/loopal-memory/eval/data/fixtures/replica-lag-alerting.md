---
name: Replica Lag Alerting
description: pg_stat_replication thresholds, page vs warn, write_lag/flush_lag/replay_lag distinction
type: reference
created_at: 2026-03-01
updated_at: 2026-05-12
ttl_days: null
related:
  - postgres-runbook
  - streaming-replication-setup
  - dashboard-metrics
---

Warn at replay_lag > 30s, page at > 5min OR write_lag > 2min on sync replica. Source query: SELECT * FROM pg_stat_replication. Dashboard panel defined in [[dashboard-metrics]]. Sync replica lag = write blocking risk; see [[streaming-replication-setup]] for the synchronous_commit semantics and [[postgres-runbook]] for failover decision tree.
