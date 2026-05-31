---
name: Scanner Replay Recovery
description: Resuming an interrupted scan from checkpoint
type: project
created_at: 2026-03-15
updated_at: 2026-04-30
ttl_days: 90
related:
  - scanner-state
  - scanner-idempotency
---

Replay reads the last checkpoint from the state table in [[scanner-state]], re-fetches the cursor position, and resumes processing. Items between the cursor and the crash point are re-processed but produce no duplicate output thanks to [[scanner-idempotency]]. Typical replay completes in < 5% of original scan time.
