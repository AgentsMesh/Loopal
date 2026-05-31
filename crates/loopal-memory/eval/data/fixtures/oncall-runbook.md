---
name: Oncall Runbook Index
description: Entry point for any paged alert — find the runbook section by rule name
type: reference
created_at: 2026-01-30
updated_at: 2026-05-18
ttl_days: null
related:
  - alert-rules-catalog
  - incident-postmortem-template
  - trace-sampling-policy
---

Runbook sections are named exactly after the alert rule (e.g. `scanner_lag_critical` → `runbook.md#scanner_lag_critical`). Each section MUST have: symptom, diagnostic queries (link to [[reference-grafana-dashboard]]), known-good remediation, and escalation path. After resolving page, file [[incident-postmortem-template]] within 24h if SEV<=2.
