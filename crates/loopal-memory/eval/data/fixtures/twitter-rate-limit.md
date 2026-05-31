---
name: Twitter Rate Limit Caps
description: Numerical rate-limit thresholds per endpoint
type: project
created_at: 2026-04-02
updated_at: 2026-05-10
ttl_days: 90
related:
  - twitter-automation
  - twitter-cooldown
---

Concrete rate caps observed in production: 50 follow actions / hour per account, 100 likes / hour, 25 DMs / hour. Beyond these you hit the soft-ban threshold described in [[twitter-cooldown]]. See [[twitter-automation]] for the policy framework that uses these numbers.
