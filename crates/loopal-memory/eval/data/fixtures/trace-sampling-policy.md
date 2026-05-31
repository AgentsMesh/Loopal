---
name: Trace Sampling Policy
description: Head vs tail sampling rules and budget per service
type: project
created_at: 2026-03-12
updated_at: 2026-05-22
ttl_days: null
related:
  - otel-trace-propagation
  - oncall-runbook
---

Default head sampling 1% at the edge; tail sampler in OTel Collector keeps 100% of error spans and any trace with span >2s. Scanner is special-cased to 10% head because its [[scanner-state]] retries need higher fidelity for debugging. Sampling decisions are encoded in the `traceparent` flags per [[otel-trace-propagation]] so downstream services don't re-decide.
