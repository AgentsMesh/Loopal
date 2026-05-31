---
name: Batch PR Approach Validated
description: Single bundled PR preferred for area-wide refactors
type: feedback
created_at: 2026-03-08
updated_at: 2026-03-08
ttl_days: null
related: []
---

**Rule**: for refactors that touch one logical area, ship a single bundled PR — don't split into per-file PRs.

**Why**: user confirmed after the loopal-memory split: splitting into 8 PRs would have been churn, not value. Reviewer context is preserved better in a single PR for area-scoped work.

**How to apply**: only split when the changes are independent (separate concerns); bundle when they share a single rationale.
