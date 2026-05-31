---
name: Streaming Replication & Failover
description: Physical streaming replication topology, sync vs async tradeoffs, Patroni failover
type: reference
created_at: 2026-01-15
updated_at: 2026-05-10
ttl_days: null
related:
  - postgres-runbook
  - replica-lag-alerting
  - pgbackrest-backup-policy
  - pitr-recovery-drill
---

Primary db-prod-01 → sync replica db-prod-02 (same AZ) → async replica db-prod-03 (cross-region). synchronous_commit=remote_apply; max_wal_senders=10; wal_keep_size=8GB. Failover orchestrated by Patroni etcd cluster — see [[postgres-runbook]] for promotion steps. Lag SLO and paging thresholds defined in [[replica-lag-alerting]].
