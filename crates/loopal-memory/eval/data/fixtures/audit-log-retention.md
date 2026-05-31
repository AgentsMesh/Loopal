---
name: Audit Log Retention
description: 18-month retention with WORM S3 + Grafana dashboard
type: reference
created_at: 2026-03-01
updated_at: 2026-05-15
ttl_days: null
related:
  - auth-architecture
  - rbac-role-matrix
  - reference-grafana-dashboard
---

Auth events (login, logout, role change, MFA enroll, key rotation) write
to s3://loopal-audit/auth/ with Object Lock in compliance mode, retention
18 months. The query layer is the auth panel of
[[reference-grafana-dashboard]]; role-change events specifically come from
[[rbac-role-matrix]] enforcement, and the whole pipeline is part of
[[auth-architecture]].
