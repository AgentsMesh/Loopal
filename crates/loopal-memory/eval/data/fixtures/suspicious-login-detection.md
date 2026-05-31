---
name: Suspicious Login Detection
description: Geo + device + velocity rules feeding step-up MFA
type: project
created_at: 2026-03-22
updated_at: 2026-05-25
ttl_days: null
related:
  - mfa-enrollment-flow
  - audit-log-retention
  - oauth2-pkce-flow
  - reference-grafana-dashboard
---

Rules: (1) impossible travel >500 km/h between successful logins, (2) new
device fingerprint + new ASN, (3) >5 failed attempts in 10 min from same
IP. Risk≥medium triggers step-up per [[mfa-enrollment-flow]]; all
decisions log to [[audit-log-retention]] and surface on
[[reference-grafana-dashboard]]. Source events come from
[[oauth2-pkce-flow]] /token endpoint.
