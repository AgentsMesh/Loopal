---
name: JWT Clock Skew Incident 2026-04
description: Postmortem — verifiers rejecting valid tokens during DST
type: feedback
created_at: 2026-04-15
updated_at: 2026-04-18
ttl_days: 365
related:
  - jwt-rs256-signing
  - audit-log-retention
---

On 2026-04-13 ~3% of /api requests returned 401 invalid_token for ~40 min
after an edge node's chrony drifted +8s; verifiers had leeway=0. Fix:
standardize leeway=30s in the shared JWT verifier wrapper used by all
services per [[jwt-rs256-signing]]. Postmortem audit trail captured in
[[audit-log-retention]]; alert added for chrony offset >2s on edge fleet.
