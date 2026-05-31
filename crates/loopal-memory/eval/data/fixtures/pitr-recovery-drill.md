---
name: Quarterly PITR Recovery Drill
description: Checklist for the quarterly point-in-time-recovery drill into a staging cluster
type: reference
created_at: 2026-04-05
updated_at: 2026-04-08
ttl_days: null
related:
  - pgbackrest-backup-policy
  - streaming-replication-setup
---

Every quarter: pick random target timestamp from last 14 days, restore to staging-db-recover, verify row counts in 5 canary tables, time the full RTO. Last drill 2026-Q1: RTO 24min (target 30min). See [[pgbackrest-backup-policy]] for stanza names; replication catch-up procedure in [[streaming-replication-setup]].
