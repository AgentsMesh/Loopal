---
name: Twitter Monitor Scraping
description: Read-side monitoring without writes
type: project
created_at: 2026-04-08
updated_at: 2026-05-08
ttl_days: 90
related:
  - twitter-automation
  - twitter-rate-limit
  - chrome-cdp
---

Read-only monitoring (search, profile fetch, follower count) has separate caps from write actions. We scrape via [[chrome-cdp]] sessions with rotating proxies. The read caps are looser than [[twitter-rate-limit]] write caps but still enforced — and they share the soft-ban state from [[twitter-automation]].
