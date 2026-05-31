---
name: Runner Autoscale Tuning
description: HRA min=2 max=24 with 90s scale-down delay after the Feb queue-storm.
type: project
created_at: 2026-02-14
updated_at: 2026-02-21
ttl_days: 180
related:
  - self-hosted-runner-pool
  - flaky-test-quarantine
---

After the Feb 12 incident where queued jobs spiked to 380 (see
[[self-hosted-runner-pool]]), we set HorizontalRunnerAutoscaler to
min=2/max=24 with `scaleDownDelaySecondsAfterScaleOut: 90`. The spike
was amplified by [[flaky-test-quarantine]] retries; reducing retry from
3 to 1 cut queue depth p99 from 380 to 47.
