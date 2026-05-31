---
name: Twitter Cooldown Procedure
description: Recovery flow after hitting Twitter soft-ban thresholds
type: project
created_at: 2026-04-03
updated_at: 2026-05-12
ttl_days: 90
related:
  - twitter-rate-limit
  - twitter-automation
---

When a worker hits the limits in [[twitter-rate-limit]], the cooldown procedure is: pause all writes for 6h, then resume at 50% throttle for 24h, then full throttle. Tracked via the cooldown_state table; see [[twitter-automation]] for which signals trigger this state.
