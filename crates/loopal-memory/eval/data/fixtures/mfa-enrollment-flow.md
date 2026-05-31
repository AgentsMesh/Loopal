---
name: MFA Enrollment & Step-Up
description: TOTP enrollment + WebAuthn step-up for sensitive ops
type: project
created_at: 2026-02-14
updated_at: 2026-05-09
ttl_days: null
related:
  - session-expiry-policy
  - rbac-role-matrix
  - suspicious-login-detection
---

Enrollment offers TOTP (RFC 6238, 30s window, SHA-1 for Authenticator
compat) plus optional WebAuthn platform authenticator; WebAuthn is required
for any role with admin:* per [[rbac-role-matrix]]. Step-up is triggered
by [[session-expiry-policy]] (>24h absolute) or by
[[suspicious-login-detection]] flagging the session as risk≥medium.
