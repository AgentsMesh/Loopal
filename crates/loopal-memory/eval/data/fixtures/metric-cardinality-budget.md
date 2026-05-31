---
name: Prometheus Metric Cardinality Budget
description: Per-service active-series cap and how we enforce it
type: project
created_at: 2026-02-18
updated_at: 2026-05-08
ttl_days: null
related:
  - structured-log-schema
  - alert-fatigue-audit
---

Each service gets 50k active series; the `prometheus_target_scrapes_exceeded_sample_limit_total` alert fires at 80%. Common offenders: putting `request_id` or `trace_id` in label values (use exemplars instead, same rule as [[structured-log-schema]]). When budget breach causes scrape failure cascade it shows up in [[alert-fatigue-audit]] as a noisy paging hotspot.
