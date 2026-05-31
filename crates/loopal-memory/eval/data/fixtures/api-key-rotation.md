---
name: API Key Rotation
description: 90-day rotation for M2M keys with dual-key grace
type: project
created_at: 2026-03-15
updated_at: 2026-05-20
ttl_days: null
related:
  - auth-architecture
  - jwt-rs256-signing
  - feedback-batch-rejected
---

Machine-to-machine API keys rotate every 90 days; both old and new keys
are accepted for a 7-day overlap window. Rate-limit responses (HTTP 429)
during rotation windows surface to the same telemetry as
[[feedback-batch-rejected]] — distinguish 401 (key expired) from 429.
Signing keys for [[jwt-rs256-signing]] rotate on the same cadence; see
[[auth-architecture]].
