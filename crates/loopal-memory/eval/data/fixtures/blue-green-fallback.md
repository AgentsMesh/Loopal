---
name: Blue-Green Fallback
description: When canary fails twice, swap full traffic via ALB target-group weights.
type: project
created_at: 2026-03-08
updated_at: 2026-04-10
ttl_days: 365
related:
  - deploy-canary-rollout
  - dashboard-deploy
---

Fallback path: ALB listener rule flips weight 100/0 from green to blue
in one API call (`modify-listener --default-actions`). We kept this
because [[deploy-canary-rollout]]'s gradual abort takes 4-7 minutes,
and the Mar 6 outage showed we need <60s recovery. Dashboard panel
added in [[dashboard-deploy]].
