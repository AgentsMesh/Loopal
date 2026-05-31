---
name: Scanner Idempotency Keys
description: How re-runs produce identical output
type: project
created_at: 2026-03-12
updated_at: 2026-04-28
ttl_days: 90
related:
  - scanner-state
  - scanner-replay
---

Each scanned item gets a content-hash key. Output writes use INSERT OR IGNORE keyed on that hash, so re-runs are no-ops on already-processed items. See [[scanner-state]] for how the state machine tracks which items belong to which scan. Replay flow in [[scanner-replay]] relies on this contract.
