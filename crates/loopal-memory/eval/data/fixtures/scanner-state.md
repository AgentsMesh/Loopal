---
name: Scanner State Machine
description: Resumable scan job state tracking
type: project
created_at: 2026-03-10
updated_at: 2026-05-01
ttl_days: 90
related:
  - scanner-idempotency
  - scanner-replay
  - user-tone-prefer-direct
---

Scan jobs persist state in a checkpoint table so they can resume after crashes. States: queued → running → checkpointing → done. Each transition is atomic. Idempotency contract in [[scanner-idempotency]] guarantees that re-running a checkpointed scan produces the same output. See [[scanner-replay]] for the recovery flow.
