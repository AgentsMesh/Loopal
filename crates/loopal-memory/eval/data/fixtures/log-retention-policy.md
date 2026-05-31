---
name: Loki Log Retention Policy
description: How long we keep logs at each tier and why
type: project
created_at: 2026-01-22
updated_at: 2026-03-15
ttl_days: null
related:
  - observability-stack
  - structured-log-schema
---

Hot tier (Loki, queryable <2s): 14 days. Warm tier (S3, queryable via `logcli` with 30s+ latency): 90 days. Anything older is dropped — financial events ship separately to BigQuery from the producer, not from logs. Costs are driven by label cardinality, see [[structured-log-schema]] for the rule against putting `user_id` in labels, and the policy is summarized in [[observability-stack]].
