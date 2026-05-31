---
name: No Mocks in Integration Tests
description: Real DB for integration tests, never mocks
type: feedback
created_at: 2026-02-05
updated_at: 2026-02-05
ttl_days: null
related: []
---

**Rule**: integration tests must hit a real database, not mocks.

**Why**: Last quarter a mocked test passed while the prod migration failed because the mock didn't enforce the new NOT NULL constraint. Mock/prod divergence hid the bug.

**How to apply**: when writing tests for any code that touches the DB schema, set up a tempfile sqlite + apply real schema; never inject a mock connection.
