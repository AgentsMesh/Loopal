---
name: Auth Architecture Overview
description: Hub doc for our auth + session security stack
type: reference
created_at: 2026-01-12
updated_at: 2026-05-18
ttl_days: null
related:
  - oauth2-pkce-flow
  - jwt-rs256-signing
  - session-expiry-policy
  - rbac-role-matrix
  - audit-log-retention
  - api-key-rotation
---

All user-facing services authenticate via [[oauth2-pkce-flow]] and issue
short-lived [[jwt-rs256-signing]] access tokens with refresh tokens stored
in httpOnly cookies; expiry rules are codified in [[session-expiry-policy]]
and role mapping lives in [[rbac-role-matrix]]. Audit events flow to S3
per [[audit-log-retention]], and machine-to-machine traffic uses keys
rotated under [[api-key-rotation]].
