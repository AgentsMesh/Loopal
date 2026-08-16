# Scoped Rust coverage gate

Run the complete fail-closed producer matrix and final gate serially with:

```bash
LOOPAL_COVERAGE_OUTPUT_DIR=/tmp/loopal-coverage \
  bash tools/coverage/run_scoped_gate.sh
```

`tools/coverage/shards.txt` is the machine-readable producer manifest used by CI and tag
release jobs. The script removes the single generated Bazel report before each producer,
runs one producer at a time, and copies its non-empty LCOV immediately before continuing.

Generate a stable base report for the exact curated producer set:

```bash
bazel coverage --jobs=2 --local_test_jobs=1 --combined_report=lcov \
  //crates/loopal-acp:loopal-acp-unit-test \
  //crates/loopal-acp:loopal-acp_test \
  //crates/loopal-agent:loopal-agent-unit-test \
  //crates/loopal-agent:loopal-agent_test \
  //crates/loopal-agent-client:loopal-agent-client-unit-test \
  //crates/loopal-agent-client:loopal-agent-client_test \
  //crates/loopal-agent-hub:loopal-agent-hub-unit-test \
  //crates/loopal-agent-server:loopal-agent-server-unit-test \
  //crates/loopal-agent-server:loopal-agent-server_test \
  //crates/loopal-backend:loopal-backend-unit-test \
  //crates/loopal-backend:loopal-backend_test \
  //crates/loopal-config:loopal-config-unit-test \
  //crates/loopal-config:loopal-config_test \
  //crates/loopal-hub-vault:loopal-hub-vault-unit-test \
  //crates/loopal-hub-vault:loopal-hub-vault_test \
  //crates/loopal-ipc:loopal-ipc-unit-test \
  //crates/loopal-ipc:loopal-ipc_test \
  //crates/loopal-mcp:loopal-mcp-unit-test \
  //crates/loopal-mcp:loopal-mcp_test \
  //crates/loopal-meta-hub:loopal-meta-hub_test \
  //crates/loopal-meta-hub:loopal-meta-hub_e2e \
  //crates/loopal-output-guard:loopal-output-guard_test \
  //crates/loopal-protocol:loopal-protocol_test \
  //crates/loopal-provider-api:loopal-provider-api-unit-test \
  //crates/loopal-provider-api:loopal-provider-api_test \
  //crates/loopal-runtime:loopal-runtime-unit-test \
  //crates/loopal-runtime:loopal-runtime_test \
  //crates/loopal-secret-client:loopal-secret-client-unit-test \
  //crates/loopal-secret-runtime:loopal-secret-runtime-unit-test \
  //crates/loopal-secret-runtime:loopal-secret-runtime_test \
  //crates/loopal-session:loopal-session-unit-test \
  //crates/loopal-session:loopal-session_test \
  //crates/loopal-storage:loopal-storage-unit-test \
  //crates/loopal-storage:loopal-storage_test \
  //crates/loopal-tool-api:loopal-tool-api-unit-test \
  //crates/loopal-tool-api:loopal-tool-api_test \
  //crates/loopal-turn:loopal-turn_test \
  //crates/loopal-tui:loopal-tui-unit-test \
  //crates/tools/filesystem/read-image:loopal-tool-read-image-test \
  //crates/tools/process/background:loopal-tool-background-unit-test \
  //crates/tools/process/background:loopal-tool-background-test \
  //crates/tools/process/bash:loopal-tool-bash-unit-test \
  //crates/tools/process/bash:loopal-tool-bash-test \
  //crates/tools/process/bash-process:loopal-tool-bash-process-test \
  //crates/loopal-vault-age:loopal-vault-age_test \
  //crates/loopal-vault-api:loopal-vault-api-unit-test \
  //crates/loopal-view-state:loopal-view-state-unit-test \
  //crates/loopal-view-state:loopal-view-state_test \
  //crates/loopal-workflow-schema:loopal-workflow-schema_test \
  //crates/tools/filesystem/fetch:loopal-tool-fetch-unit-test \
  //crates/tools/filesystem/fetch:loopal-tool-fetch-test \
  //:loopal-unit-test \
  --test_arg=--test-threads=1
cp bazel-out/_coverage/_coverage_report.dat /tmp/loopal-base-general.lcov

bazel coverage --jobs=1 --local_test_jobs=1 --combined_report=lcov \
  //crates/loopal-tui:loopal-tui_test \
  --test_arg=--test-threads=1
cp bazel-out/_coverage/_coverage_report.dat /tmp/loopal-base-tui.lcov

bazel coverage --jobs=2 --local_test_jobs=1 --combined_report=lcov \
  //crates/loopal-agent-hub:loopal-agent-hub_test \
  --test_arg=--test-threads=2
cp bazel-out/_coverage/_coverage_report.dat /tmp/loopal-base-hub.lcov

bazel coverage --jobs=2 --local_test_jobs=1 --combined_report=lcov \
  //:bootstrap_start_root_coverage_test \
  //:bootstrap_workflow_runtime_coverage_test \
  //:bootstrap_lifecycle_test
cp bazel-out/_coverage/_coverage_report.dat /tmp/loopal-base-bootstrap-lifecycle.lcov

bazel coverage --jobs=2 --local_test_jobs=1 --combined_report=lcov \
  //:bootstrap_typestate_e2e_test \
  --test_arg=--test-threads=1
cp bazel-out/_coverage/_coverage_report.dat /tmp/loopal-base-bootstrap-typestate.lcov

cp /tmp/loopal-base-general.lcov /tmp/loopal-base.lcov
chmod u+w /tmp/loopal-base.lcov
cat /tmp/loopal-base-tui.lcov \
  /tmp/loopal-base-hub.lcov \
  /tmp/loopal-base-bootstrap-lifecycle.lcov \
  /tmp/loopal-base-bootstrap-typestate.lcov \
  >> /tmp/loopal-base.lcov
```

The subprocess-heavy TUI integration suite and real-process Hub and bootstrap producers run
separately because coverage instrumentation magnifies startup cost and their wall-clock
handshakes become unreliable when they share local test slots. The lifecycle producer also
runs ignored in-process bootstrap tests that child-process profile harvesting cannot replace.
LCOV duplicate source records merge by source identity.
Keep the job caps and producer isolation; do not raise behavioral timeouts to compensate
for an oversubscribed coverage batch.

Generate real Rust branch reports with `--config=rust_branch` in bounded target shards,
copying each report before the next `bazel coverage` invocation overwrites it. Do not put
a large target into a shard when LLVM's coverage reader crashes on that binary; split its
coverage-only producer until every shard exports successfully. Then evaluate all reports
in one fail-closed invocation:

```bash
bazel coverage --config=rust_branch --jobs=2 --local_test_jobs=1 \
  //crates/loopal-output-guard:loopal-output-guard_test \
  //crates/loopal-protocol:loopal-protocol_test \
  --test_arg=--test-threads=1 --test_output=errors
cp bazel-out/_coverage/_coverage_report.dat /tmp/loopal-branch-01-protocol-output.lcov

bazel coverage --config=rust_branch --jobs=2 --local_test_jobs=1 \
  //crates/loopal-storage:loopal-storage-unit-test \
  //crates/loopal-storage:loopal-storage_test \
  //crates/loopal-config:loopal-config-unit-test \
  //crates/loopal-config:loopal-config_test \
  //crates/loopal-workflow-schema:loopal-workflow-schema_test \
  --test_arg=--test-threads=1 --test_output=errors
cp bazel-out/_coverage/_coverage_report.dat /tmp/loopal-branch-02-workflow-storage-config.lcov

bazel coverage --config=rust_branch --jobs=2 --local_test_jobs=1 \
  //crates/loopal-runtime:loopal-runtime-unit-test \
  //crates/loopal-runtime:loopal-runtime_test \
  //crates/loopal-tool-api:loopal-tool-api-unit-test \
  //crates/loopal-tool-api:loopal-tool-api_test \
  //crates/loopal-provider-api:loopal-provider-api-unit-test \
  //crates/loopal-provider-api:loopal-provider-api_test \
  --test_arg=--test-threads=1 --test_output=errors
cp bazel-out/_coverage/_coverage_report.dat /tmp/loopal-branch-03-runtime-toolapi.lcov

bazel coverage --config=rust_branch --jobs=2 --local_test_jobs=1 \
  //crates/loopal-mcp:loopal-mcp-unit-test \
  //crates/loopal-mcp:loopal-mcp_test \
  //crates/loopal-secret-client:loopal-secret-client-unit-test \
  //crates/loopal-secret-runtime:loopal-secret-runtime-unit-test \
  //crates/loopal-secret-runtime:loopal-secret-runtime_test \
  //crates/loopal-hub-vault:loopal-hub-vault-unit-test \
  //crates/loopal-hub-vault:loopal-hub-vault_test \
  //crates/loopal-vault-age:loopal-vault-age_test \
  //crates/loopal-vault-api:loopal-vault-api-unit-test \
  --test_arg=--test-threads=1 --test_output=errors
cp bazel-out/_coverage/_coverage_report.dat /tmp/loopal-branch-04-mcp-secret-vault.lcov

bazel coverage --config=rust_branch --jobs=2 --local_test_jobs=1 \
  //crates/loopal-agent:loopal-agent-unit-test \
  //crates/loopal-agent:loopal-agent_test \
  //crates/loopal-agent-server:loopal-agent-server-unit-test \
  //crates/loopal-agent-server:loopal-agent-server_test \
  --test_arg=--test-threads=1 --test_output=errors
cp bazel-out/_coverage/_coverage_report.dat /tmp/loopal-branch-05-agent-server.lcov

bazel coverage --config=rust_branch --jobs=2 --local_test_jobs=1 \
  //crates/loopal-backend:loopal-backend-unit-test \
  //crates/loopal-backend:loopal-backend_test \
  --test_arg=--test-threads=1 --test_output=errors
cp bazel-out/_coverage/_coverage_report.dat /tmp/loopal-branch-06-backend.lcov

bazel coverage --config=rust_branch --jobs=2 --local_test_jobs=1 \
  //crates/tools/process/background:loopal-tool-background-unit-test \
  //crates/tools/process/background:loopal-tool-background-test \
  //crates/tools/process/bash:loopal-tool-bash-unit-test \
  //crates/tools/process/bash:loopal-tool-bash-test \
  //crates/tools/process/bash-process:loopal-tool-bash-process-test \
  --test_arg=--test-threads=1 --test_output=errors
cp bazel-out/_coverage/_coverage_report.dat /tmp/loopal-branch-07-process-tools.lcov

bazel coverage --config=rust_branch --jobs=2 --local_test_jobs=1 \
  //crates/loopal-acp:loopal-acp-unit-test \
  //crates/loopal-acp:loopal-acp_test \
  //crates/loopal-agent-client:loopal-agent-client-unit-test \
  //crates/loopal-agent-client:loopal-agent-client_test \
  //crates/loopal-ipc:loopal-ipc-unit-test \
  //crates/loopal-ipc:loopal-ipc_test \
  //crates/loopal-meta-hub:loopal-meta-hub_test \
  //crates/loopal-meta-hub:loopal-meta-hub_e2e \
  //crates/loopal-session:loopal-session-unit-test \
  //crates/loopal-session:loopal-session_test \
  //crates/loopal-turn:loopal-turn_test \
  //crates/loopal-tui:loopal-tui-unit-test \
  //crates/loopal-tui:loopal-tui_test \
  //crates/loopal-view-state:loopal-view-state-unit-test \
  //crates/loopal-view-state:loopal-view-state_test \
  --test_arg=--test-threads=1 --test_output=errors
cp bazel-out/_coverage/_coverage_report.dat /tmp/loopal-branch-08-edges-view.lcov

bazel coverage --config=rust_branch --jobs=2 --local_test_jobs=1 \
  //crates/loopal-agent-hub:loopal-agent-hub-unit-test \
  //crates/loopal-agent-hub:loopal-agent-hub_test \
  --test_arg=--test-threads=2 --test_output=errors
cp bazel-out/_coverage/_coverage_report.dat /tmp/loopal-branch-09-agent-hub.lcov

bazel coverage --config=rust_branch --jobs=2 --local_test_jobs=1 \
  //crates/tools/filesystem/fetch:loopal-tool-fetch-unit-test \
  //crates/tools/filesystem/fetch:loopal-tool-fetch-test \
  //crates/tools/filesystem/read-image:loopal-tool-read-image-test \
  --test_arg=--test-threads=1 --test_output=errors
cp bazel-out/_coverage/_coverage_report.dat /tmp/loopal-branch-10-filesystem-tools.lcov

bazel coverage --config=rust_branch --jobs=2 --local_test_jobs=1 \
  //:bootstrap_start_root_coverage_test \
  //:bootstrap_workflow_runtime_coverage_test \
  //:loopal-unit-test \
  //:bootstrap_lifecycle_test \
  //:bootstrap_typestate_e2e_test \
  --test_output=errors
cp bazel-out/_coverage/_coverage_report.dat /tmp/loopal-branch-11-bootstrap.lcov
```

The current bounded branch matrix is:

1. `protocol-output`: output guard and protocol tests.
2. `workflow-storage-config`: storage, config, and workflow-schema tests.
3. `runtime-toolapi`: runtime, tool-API, and provider-API tests.
4. `mcp-secret-vault`: MCP, secret-client, secret-runtime, Hub-vault, vault-age, and vault-API tests.
5. `agent-server`: Agent and AgentServer unit and integration tests.
6. `backend`: backend unit and integration tests.
7. `process-tools`: background, Bash, and Bash-process tests.
8. `edges-view`: ACP, AgentClient, IPC, MetaHub, Session, Turn, TUI, and ViewState tests.
9. `agent-hub`: AgentHub unit and integration tests.
10. `filesystem-tools`: fetch and read-image tests.
11. `bootstrap`: the dedicated root-start and workflow-runtime coverage producers,
    root unit, bootstrap lifecycle, and bootstrap typestate tests.

Copy them as `/tmp/loopal-branch-01-protocol-output.lcov` through
`/tmp/loopal-branch-11-bootstrap.lcov`. Run each shard with the job caps shown above;
`//:bootstrap_start_root_coverage_test` and `//:bootstrap_workflow_runtime_coverage_test`
compile the real typestate source files through small deterministic harnesses so branch export
does not need to reproduce process and IPC failures; the harnesses supply only the boundary
types while the production sources remain the instrumented implementations. The bootstrap
lifecycle target already fixes its own Rust test thread count, so do not add a global
`--test_arg=--test-threads` to that shard.

After every producer has completed, run the final gate explicitly:

```bash
bazel run //tools/coverage:gate -- \
  /tmp/loopal-base.lcov \
  /tmp/loopal-branch-01-protocol-output.lcov \
  /tmp/loopal-branch-02-workflow-storage-config.lcov \
  /tmp/loopal-branch-03-runtime-toolapi.lcov \
  /tmp/loopal-branch-04-mcp-secret-vault.lcov \
  /tmp/loopal-branch-05-agent-server.lcov \
  /tmp/loopal-branch-06-backend.lcov \
  /tmp/loopal-branch-07-process-tools.lcov \
  /tmp/loopal-branch-08-edges-view.lcov \
  /tmp/loopal-branch-09-agent-hub.lcov \
  /tmp/loopal-branch-10-filesystem-tools.lcov \
  /tmp/loopal-branch-11-bootstrap.lcov
```

The first input is the only source of line, function, and region counts. Later inputs
contribute only branch records, so branch runs cannot inflate base coverage. Their union
must contain every path in `included_sources.txt`; a missing or crashed shard therefore
fails before thresholds are evaluated. The gate accepts at most 128 reports.

## Curated scope

`included_sources.txt` contains materially changed Stage 0/workflow behavior, not
historical glue. It covers the workflow model/reducer/validation, exact-generation Hub
registry and spawn admission, permission-intent preparation/digest/input validation and
effect execution, protected vault audit, journal path safety, serialized process-output
capture/redaction, private Bash logs, cancellation-safe owner transfer, Unix process-group
and Windows Job containment, guarded background retrieval/adoption, workflow
configuration, coordinator admission and recovery, identity-bound journal traversal and
repair, workflow terminal turn identity, view projection/TUI rendering, and production audit
bootstrap.

Run `bazel run //tools/coverage:scope_review` before handoff. It examines working-tree
changes plus committed changes since `origin/main` across those security/workflow
boundaries; set `LOOPAL_COVERAGE_BASE_REF` when the review base differs. Every candidate
must be in `included_sources.txt` or have a rationale plus its current FNV-1a content
hash in `scope_exclusions.txt`; editing an excluded file makes the hash stale and fails
review. This prevents commits from hiding omissions without diluting the gate with
unrelated crates or tests.

`critical_functions.txt` contains names verified against real Bazel LCOV symbols. It
must grow with permission denial/digest, ACL, final redaction/sink, cancellation,
stale-completion, retry, coordinator, and recovery functions as those categories land.
Aggregate Rust function coverage merges monomorphs by source line and excludes `_RNC`
compiler-generated closure/coroutine bodies; named critical checks still match the full
LCOV symbol set and fail if the source function is missing or uncovered.

## Policy

The `rust_branch` config is coverage-only. It selects Rust 1.92.0/LLVM 21.1.3 for both
the instrumented producer and the rules_rust collector, enables real Rust branch mapping,
and pairs Bazel's fetch-all and split-postprocessing flags so successful per-test
collectors can feed the final combined LCOV. Ordinary build and test commands remain on
the release Rust 1.94.0 toolchain and stable flags. The two toolchain selectors are
mutually exclusive; branch coverage never depends on registration order or a local rustup
installation.

The coverage-only pin avoids the upstream `getInstantiationGroups` exporter crash tracked
by rust-lang/rust#157358 and llvm/llvm-project#189169. Rust 1.94's LLVM 21.1.8 producer
triggers the crash for async/generic branch maps, and using an older exporter on a newer
producer is not a supported workaround. Branch reports must therefore all be regenerated
with the checked-in 1.92 producer/collector pair; do not mix an older shard into a gate
run. Native branch toolchains are registered for arm64/x86-64 macOS, Linux, and Windows.
Other hosts intentionally fail toolchain resolution instead of falling back to 1.94.

The patched rules_rust collector restores owner-write permission on the split coverage
directory (Bazel #28310), removes its temporary indexed profile after a failed
`llvm-cov export`, and does not publish partial stdout as `coverage.dat`. Bazel 8.1's
generic split-postprocessing wrapper does not propagate a language collector's status,
so a shard command can still return success with only baseline data. Keep branch
producers bounded so each collector's provenance can be checked and real-process tests
remain isolated.
Never treat `bazel coverage` alone as the release gate: the final gate fails closed if the
combined scoped Rust LCOV has no `BRDA` records or if the branch-report union omits any
scoped source.

The gate requires global line, function, and region coverage strictly above 95%, branch
coverage at least 90%, and line coverage of at least 90% in each scoped file. It rejects
missing files, empty scoped data, malformed relevant records, and reports without
function or branch data.

LCOV has no standard LLVM region record. For each file independently, the gate uses
`RG:start_line,start_column,end_line,end_column,count` when present; otherwise that
file's `DA` records are the deterministic region proxy. Mixed reports aggregate both
and print how many files used each policy, so ordinary Bazel LCOV never skips regions.
