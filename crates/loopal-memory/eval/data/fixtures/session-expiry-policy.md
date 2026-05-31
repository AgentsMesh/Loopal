---
name: Session Expiry Policy
description: TTLs for access / refresh / idle / absolute sessions
type: project
created_at: 2026-02-01
updated_at: 2026-05-03
ttl_days: null
related:
  - auth-architecture
  - jwt-rs256-signing
  - mfa-enrollment-flow
---

Access JWT TTL = 15 min, refresh TTL = 14 days sliding, idle timeout = 30
min in admin UI, absolute cap = 24h after which re-auth + MFA is required.
The absolute cap interacts with [[mfa-enrollment-flow]] (step-up required
if elapsed > 24h) and the underlying access tokens follow
[[jwt-rs256-signing]]; see [[auth-architecture]] for the full picture.
