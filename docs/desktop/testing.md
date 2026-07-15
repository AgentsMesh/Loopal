# Loopal Desktop verification

Every formal command starts with Bazel. There are no npm/pnpm build or test scripts,
and Playwright does not download a browser at test time.

## Primary targets

```bash
bazel build //apps/desktop:typecheck
bazel build //apps/desktop:out
bazel test //apps/desktop:unit --test_output=errors
bazel test //apps/desktop:coverage --test_output=errors
bazel test //apps/desktop:e2e --test_tag_filters=e2e
bazel test //apps/desktop:e2e_host --test_tag_filters=e2e
bazel test //apps/desktop:e2e_llm_backend --test_tag_filters=e2e
bazel test //crates/loopal-mock-llm:loopal-mock-llm_test
bazel test //apps/desktop:staging_smoke -c opt --test_output=errors
```

Use [the E2E contract](./e2e-contract.md) to select among fake, real Host, and
provider-boundary suites. Packaging ownership and `//apps/desktop:dist` are documented
in [build and release](./build-and-release.md).

## Unit ownership

Desktop unit tests are colocated with production TypeScript whenever they exercise one
module or feature. Shared reusable fixtures and adapters live under:

```text
apps/desktop/test/
  fixtures/workbench/
  support/backend/
  support/ipc/
  support/workbench/
  setup.ts
```

Units cover lifecycle/event/cancellation primitives, channel framing and cancellation,
Host generations and timeout recovery, Runtime quotas and deduplication, backend
projection/resync, schema validation, UI controllers, directory safety, attention,
settings, Session panels, and Federation aggregation/routing.

Feature tests and CSS move with their `workbench/contrib` feature. Cross-feature shell
tests remain in `workbench/browser`. The physical ownership rules are in
[repository layout](../repository-layout.md).

## Coverage

`//apps/desktop:coverage` enforces aggregate floors of 95% statements, 90% branches,
94% functions, and 95% lines across included production sources. Reports belong to
Bazel's undeclared test output directory, not `apps/desktop/coverage` or any other
source path.

Coverage is a floor, not a substitute for boundary tests. A schema, generation,
shutdown, authorization, or redaction change needs an explicit negative case.

## Rust gates for Desktop boundaries

The narrow Rust targets exercise the sidecar behaviors Desktop depends on:

```bash
bazel test //crates/loopal-workspace:loopal-workspace-unit-test
bazel test //crates/loopal-workspace:loopal-workspace_test
bazel test //crates/loopal-backend:loopal-backend_test
bazel test //crates/loopal-ipc:loopal-ipc-unit-test
bazel test //crates/loopal-protocol:loopal-protocol_test
bazel test //crates/loopal-view-state:loopal-view-state_test
bazel test //crates/loopal-runtime:loopal-runtime_test
bazel test //crates/loopal-session:loopal-session_test
bazel test //crates/loopal-agent-hub:loopal-agent-hub_test
bazel test //:desktop_serve_e2e_test
```

These cover root confinement, symlink escape, bounded I/O, atomic writes, Session
catalog scope, Hub ACL, request lanes, projections, attention, process startup, and
disconnect cleanup. Raw PTY methods are absent from the Desktop ACL.

Rust formatting and linting also run through Bazel:

```bash
bazel build //crates/loopal-workspace:all //crates/loopal-agent-hub:all \
  //crates/loopal-ipc:all //crates/loopal-backend:all --config=rustfmt
bazel build //crates/loopal-workspace:all //crates/loopal-agent-hub:all \
  //crates/loopal-ipc:all //crates/loopal-backend:all --config=clippy
```

## Handoff hygiene

Before handoff:

1. Run the narrow typecheck/unit target for every changed layer.
2. Run the E2E target that owns the changed process or provider boundary.
3. Run `git diff --check`.
4. Audit handwritten changed production/test files against the 200-line preference.
5. Keep traces, screenshots, videos, coverage, and package output in Bazel outputs.
