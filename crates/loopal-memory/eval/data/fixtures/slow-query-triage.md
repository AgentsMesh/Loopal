---
name: Slow Query Triage Playbook
description: pg_stat_statements + auto_explain workflow when p99 latency alarms fire
type: reference
created_at: 2026-02-02
updated_at: 2026-05-18
ttl_days: null
related:
  - postgres-runbook
  - vacuum-tuning-guide
  - index-bloat-investigation
  - dashboard-metrics
---

When p99 query time > 500ms for 5min, pull top-20 from pg_stat_statements ordered by total_exec_time. auto_explain.log_min_duration=1000ms in prod. Cross-reference with [[dashboard-metrics]] for I/O saturation. Common culprits: missing index (see [[index-bloat-investigation]]) or autovacuum starvation ([[vacuum-tuning-guide]]). Escalation path lives in [[postgres-runbook]].
