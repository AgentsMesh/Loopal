---
name: Observability Stack Overview
description: Hub doc for our logging, tracing, alerting stack and how the pieces connect
type: reference
created_at: 2026-01-10
updated_at: 2026-05-12
ttl_days: null
related:
  - structured-log-schema
  - otel-trace-propagation
  - alert-rules-catalog
  - slo-definitions
  - log-retention-policy
---

We run Vector → Loki for logs, OTel Collector → Tempo for traces, and Prometheus → Alertmanager → PagerDuty for alerts. All three feed [[reference-grafana-dashboard]]. Every service MUST emit logs per [[structured-log-schema]] and propagate trace context per [[otel-trace-propagation]]; alert wiring is owned by [[alert-rules-catalog]] and budgets come from [[slo-definitions]].
