---
name: Incident Postmortem Template
description: Blameless postmortem format we use for SEV1/SEV2 incidents
type: reference
created_at: 2026-02-14
updated_at: 2026-04-30
ttl_days: null
related:
  - slo-definitions
  - oncall-runbook
---

Sections: timeline (UTC, trace_ids from [[otel-trace-propagation]]), impact (users affected + error budget burned per [[slo-definitions]]), root cause (5-whys), action items (with owner + due date + Jira link). Published in `docs/postmortems/YYYY-MM-DD-slug.md` and reviewed in the next ops weekly. Cross-reference the relevant runbook section in [[oncall-runbook]] so future oncall finds prior context.
