---
name: OAuth2 PKCE Flow
description: Authorization Code + PKCE for SPA and mobile clients
type: reference
created_at: 2026-01-20
updated_at: 2026-04-02
ttl_days: null
related:
  - auth-architecture
  - jwt-rs256-signing
  - suspicious-login-detection
---

SPA clients (web + iOS) use Authorization Code with PKCE (S256), code
verifier 64 bytes, code_challenge sent on /authorize and verifier on
/token; we reject plain challenges per RFC 7636. Tokens minted here are
the access JWTs described in [[jwt-rs256-signing]], and login telemetry
is consumed by [[suspicious-login-detection]] — see [[auth-architecture]]
for the surrounding stack.
