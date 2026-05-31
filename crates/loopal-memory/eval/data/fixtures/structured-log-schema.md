---
name: Structured Log Schema (JSON)
description: Required fields for every log line emitted by Loopal services
type: reference
created_at: 2026-01-12
updated_at: 2026-04-28
ttl_days: null
related:
  - observability-stack
  - log-retention-policy
  - metric-cardinality-budget
---

Every log line MUST be JSON with `ts` (RFC3339), `level`, `service`, `trace_id`, `span_id`, `msg`, and optional `attrs` (flat map, no nested objects). Promtail drops lines missing `trace_id` to keep [[log-retention-policy]] costs bounded, and high-cardinality fields like `user_id` go into `attrs` not labels — see [[metric-cardinality-budget]]. This schema is the single hop between [[observability-stack]] and Loki query UX.
