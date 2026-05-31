---
name: Twitter Long-Tweet Handling
description: Splitting long content across multiple tweets
type: project
created_at: 2026-04-05
updated_at: 2026-04-25
ttl_days: 90
related:
  - twitter-automation
---

Tweets over 280 chars must be split into thread chunks. We use sentence-boundary aware splitting with a 240-char target to leave room for thread numbering. See [[twitter-automation]] for the rate-limit interplay when posting threads (each tweet counts separately).
