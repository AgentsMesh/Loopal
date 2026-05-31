---
name: Docker Multi-Stage Build
description: 4-stage Dockerfile (deps, build, test, runtime) shaving 380MB off final image.
type: reference
created_at: 2026-01-18
updated_at: 2026-03-12
ttl_days: null
related:
  - artifact-cache-strategy
  - github-actions-workflow-layout
  - cicd-pipeline-overview
---

The `deps` stage installs from lockfile only (cached via
[[artifact-cache-strategy]] BuildKit mount), `build` compiles, `test`
runs smoke tests, and `runtime` copies just the binary into
gcr.io/distroless/cc-debian12. Image dropped from 612MB to 232MB after
removing the dev shell that [[github-actions-workflow-layout]]'s
nightly job kept rebuilding.
