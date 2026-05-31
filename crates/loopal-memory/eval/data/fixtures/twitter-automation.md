---
name: Twitter Automation Policy
description: Rate-limit-aware Twitter/X automation rules
type: project
created_at: 2026-04-01
updated_at: 2026-05-15
ttl_days: 90
related:
  - twitter-rate-limit
  - twitter-cooldown
  - chrome-cdp
---

Twitter automation must respect platform rate limits to avoid soft bans. Reference [[twitter-rate-limit]] for the exact numerical caps and [[twitter-cooldown]] for the recovery procedure when caps are hit. All scripted browser sessions use [[chrome-cdp]] for protocol-level control. Never spam endpoints — a soft ban kills the worker for 24h+.
