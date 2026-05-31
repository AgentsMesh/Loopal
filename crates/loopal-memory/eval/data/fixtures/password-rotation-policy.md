---
name: Password Rotation Policy
description: NIST 800-63B aligned — no forced periodic rotation
type: project
created_at: 2026-03-10
updated_at: 2026-04-22
ttl_days: null
related:
  - mfa-enrollment-flow
  - suspicious-login-detection
  - auth-architecture
---

We follow NIST 800-63B: no forced periodic password change; rotation is
triggered only on breach signal from haveibeenpwned k-anonymity check or
from [[suspicious-login-detection]]. Users with WebAuthn enrolled per
[[mfa-enrollment-flow]] are exempt from password complexity prompts. The
overall stance is documented in [[auth-architecture]].
