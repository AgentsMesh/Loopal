---
name: 2026-03-14 Connection Storm Postmortem
description: Postmortem of the pgbouncer pool exhaustion incident triggered by a runaway migration
type: feedback
created_at: 2026-03-16
updated_at: 2026-03-20
ttl_days: null
related:
  - pgbouncer-pool-sizing
  - schema-migration-flyway
  - feedback-batch-rejected
---

Migration V20260314_add_user_email_index.sql forgot CONCURRENTLY → AccessExclusiveLock 12min → pgbouncer pool drained → app workers retry-stormed → cascading FK validation lock. Drove [[pgbouncer-pool-sizing]] from 48→24 (less queue room, faster fail) and made CONCURRENTLY mandatory in [[schema-migration-flyway]]. Related write-contention pattern documented in [[feedback-batch-rejected]].
