---
name: Prometheus Alert Rules Catalog
description: Where the alert rules live, naming convention, and review cadence
type: reference
created_at: 2026-02-02
updated_at: 2026-05-14
ttl_days: null
related:
  - observability-stack
  - alert-fatigue-audit
  - oncall-runbook
  - slo-definitions
---

Rules live in `ops/prometheus/rules/*.yaml`, named `<service>_<symptom>_<severity>` (e.g. `scanner_lag_critical`). Every rule MUST link to a runbook section in [[oncall-runbook]] and be backed by a budget in [[slo-definitions]]. Quarterly we run [[alert-fatigue-audit]] to retire rules with >40% false-positive rate.
