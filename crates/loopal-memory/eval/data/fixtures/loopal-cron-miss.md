---
name: Loopal Cron Miss Handling
description: What happens when scheduled jobs miss their window
type: project
created_at: 2026-04-10
updated_at: 2026-04-18
ttl_days: 90
related: []
---

If the agent process is offline at a scheduled cron time, the job is recorded as "missed" not silently dropped. On next startup, missed durable jobs are surfaced for catch-up review. Non-durable (session-only) jobs are gone with the process and are not recovered.
