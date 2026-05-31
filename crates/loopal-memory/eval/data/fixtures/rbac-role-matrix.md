---
name: RBAC Role Matrix
description: Canonical role → permission mapping
type: project
created_at: 2026-02-20
updated_at: 2026-04-28
ttl_days: null
related:
  - auth-architecture
  - mfa-enrollment-flow
  - audit-log-retention
---

Roles: viewer, operator, admin, security-admin. admin:* and
security-admin:* require WebAuthn step-up per [[mfa-enrollment-flow]]; all
role grants are written to the audit trail described in
[[audit-log-retention]]. The matrix is enforced at the API gateway level —
see [[auth-architecture]] for where it sits in the request path.
