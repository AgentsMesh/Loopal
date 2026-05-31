---
name: Index Bloat Investigation
description: pgstattuple + REINDEX CONCURRENTLY workflow for bloated indexes
type: reference
created_at: 2026-03-22
updated_at: 2026-04-18
ttl_days: 180
related:
  - vacuum-tuning-guide
  - slow-query-triage
---

Run pgstattuple_approx on suspect index; bloat > 40% triggers REINDEX CONCURRENTLY. Cannot reindex while [[vacuum-tuning-guide]] autovacuum holds ShareUpdateExclusiveLock — schedule overnight. Symptom usually surfaces via [[slow-query-triage]] as a previously fast lookup now doing Bitmap Index Scan with 10x heap fetches.
