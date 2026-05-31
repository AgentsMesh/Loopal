---
name: Flaky Test Quarantine
description: Auto-tag tests with >2% failure rate over 200 runs; quarantined tests run nightly only.
type: project
created_at: 2026-02-18
updated_at: 2026-04-20
ttl_days: null
related:
  - test-parallelization-shards
  - cicd-pipeline-overview
  - feedback-no-mocks-in-tests
---

A nightly job scans the last 200 CI runs from BigQuery export, marks
any test with >2% failure rate, and adds `#[ignore = "quarantine"]`
via codemod. Quarantined tests still run in nightly.yml so we don't
lose signal. Many quarantines trace back to network mocks — see
[[feedback-no-mocks-in-tests]]. Sharding config in
[[test-parallelization-shards]].
