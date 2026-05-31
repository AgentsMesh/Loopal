---
name: PgBouncer Pool Sizing
description: Transaction pooling pool_size formula, why session pooling is banned for app traffic
type: reference
created_at: 2026-01-20
updated_at: 2026-03-18
ttl_days: null
related:
  - postgres-runbook
  - connection-storm-incident
  - slow-query-triage
---

Mode=transaction. default_pool_size = (cpu_cores * 2) + effective_spindle_count, currently 24 per backend. max_client_conn=4000 across 8 pgbouncer instances behind LB. See [[connection-storm-incident]] for the 2026-03 outage that drove this number down from 48. Session pooling is forbidden — prepared statements break it; long transactions starve workers (see [[slow-query-triage]]).
