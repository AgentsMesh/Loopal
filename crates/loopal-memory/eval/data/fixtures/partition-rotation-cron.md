---
name: Monthly Partition Rotation
description: pg_partman create_parent + retention cron for time-series tables
type: reference
created_at: 2026-02-20
updated_at: 2026-05-05
ttl_days: null
related:
  - postgres-runbook
  - vacuum-tuning-guide
  - schema-migration-flyway
---

events and api_logs partitioned monthly via pg_partman, premake=3, retention=12 months. Cron runs 2026-style cron "17 3 * * *" calling run_maintenance_proc(). Drop-old policy detaches partitions before DROP TABLE — must happen before [[vacuum-tuning-guide]] kicks off heavy autovacuum at month boundary. Schema changes to partitioned tables must follow [[schema-migration-flyway]] ALTER TABLE ONLY discipline.
