---
name: Session Fixation Regression Test
description: Regenerate session id on privilege escalation
type: feedback
created_at: 2026-04-25
updated_at: 2026-05-12
ttl_days: 180
related:
  - session-expiry-policy
  - mfa-enrollment-flow
  - rbac-role-matrix
---

Integration test asserts that on successful MFA step-up
([[mfa-enrollment-flow]]) the session cookie value changes — previously we
only bumped a claim, which allowed a fixation vector when an attacker
pre-seeded the cookie. Test lives in
auth-service/tests/session_fixation_test.rs; rule applies to any
[[rbac-role-matrix]] role transition and is enforced by
[[session-expiry-policy]] hooks.
