---
name: Postgres Production Runbook
description: Hub for all PostgreSQL operational procedures, on-call playbooks, and SLO definitions
type: reference
created_at: 2026-01-08
updated_at: 2026-05-22
ttl_days: null
related:
  - pgbackrest-backup-policy
  - streaming-replication-setup
  - pgbouncer-pool-sizing
  - slow-query-triage
  - vacuum-tuning-guide
  - partition-rotation-cron
  - replica-lag-alerting
---

Central index for prod Postgres 16 cluster (db-prod-01 primary + 2 replicas). For HA failover see [[streaming-replication-setup]]; for backup/PITR see [[pgbackrest-backup-policy]]; pool sizing rules live in [[pgbouncer-pool-sizing]]. On-call must read [[slow-query-triage]] before paging the DBA.
