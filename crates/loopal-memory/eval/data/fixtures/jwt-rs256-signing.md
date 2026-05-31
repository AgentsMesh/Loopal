---
name: JWT RS256 Signing & Verification
description: Asymmetric JWT signing with JWKS rotation
type: reference
created_at: 2026-01-22
updated_at: 2026-05-10
ttl_days: null
related:
  - auth-architecture
  - oauth2-pkce-flow
  - api-key-rotation
---

Access tokens are RS256 with 2048-bit keys; JWKS served at
/.well-known/jwks.json with kid rotated every 30 days while keeping the
prior key valid for 24h grace. Verifiers MUST check iss=auth.loopal.io,
aud=api, and exp; symmetric HS256 is forbidden in code review. Issuance
happens after [[oauth2-pkce-flow]] succeeds; key rotation cadence mirrors
[[api-key-rotation]] and feeds [[auth-architecture]].
