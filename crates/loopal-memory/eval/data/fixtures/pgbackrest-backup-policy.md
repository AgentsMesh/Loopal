---
name: pgBackRest Backup & PITR Policy
description: Full/diff/incr schedule, retention, and PITR recovery RTO/RPO targets
type: reference
created_at: 2026-01-12
updated_at: 2026-04-30
ttl_days: null
related:
  - postgres-runbook
  - streaming-replication-setup
  - pitr-recovery-drill
---

Weekly full Sunday 02:00 UTC, daily diff, hourly incr, retained 14 days on S3 (loopal-pgbackup-prod). RPO = 1h, RTO = 30min for PITR. See [[postgres-runbook]] for escalation; [[pitr-recovery-drill]] must run quarterly. WAL archive_command goes through pgbackrest-stanza-create — never set archive_mode=off without coordinating with [[streaming-replication-setup]].
