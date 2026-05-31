---
name: OpenTelemetry Trace Propagation
description: W3C traceparent header handling across HTTP, gRPC, and async job queues
type: reference
created_at: 2026-01-18
updated_at: 2026-05-03
ttl_days: null
related:
  - observability-stack
  - trace-sampling-policy
  - structured-log-schema
---

Use W3C `traceparent` on every HTTP edge; gRPC clients inject via the OTel interceptor. For async work (Sidekiq, our internal `scanner-job` queue) we serialize `traceparent` into the job payload so [[scanner-state]] retries inherit the original trace. Sampling decisions live in [[trace-sampling-policy]] and the `trace_id` field is required by [[structured-log-schema]] to correlate logs.
