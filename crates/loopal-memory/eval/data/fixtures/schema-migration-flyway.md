---
name: Schema Migration Workflow (Flyway)
description: Online migration rules — concurrent index, NOT VALID FK, lock_timeout discipline
type: reference
created_at: 2026-03-10
updated_at: 2026-05-20
ttl_days: null
related:
  - postgres-runbook
  - partition-rotation-cron
  - connection-storm-incident
---

All DDL via Flyway V{ts}__{desc}.sql; SET lock_timeout='3s' at top of every migration. CREATE INDEX must be CONCURRENTLY; FK adds use NOT VALID then VALIDATE separately. AccessExclusiveLock for >3s aborts and rolls back. See [[connection-storm-incident]] for the migration that ignored this rule and caused 12min outage. Partitioned-table DDL rules in [[partition-rotation-cron]]; runbook entry in [[postgres-runbook]].
