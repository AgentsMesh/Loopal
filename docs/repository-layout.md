# Repository layout and dependency boundaries

This document is the source of truth for Loopal's physical repository layout and
allowed dependency direction. Semantic runtime decisions live in
[the architecture principles](./architecture.md); Desktop-specific documents are
indexed from [Desktop documentation](./desktop/README.md).

Loopal is Bazel-only. There is no Cargo workspace or checked-in `Cargo.toml` /
`Cargo.lock`. `BUILD.bazel`, `MODULE.bazel`, and `MODULE.bazel.lock` define the build
graph and external Rust toolchain/dependencies. pnpm resolves the JavaScript lockfile,
but Bazel owns Desktop build, test, E2E, staging, and packaging actions.

## Repository root

```text
BUILD.bazel                 root `loopal` binary and root test macros
MODULE.bazel                Bazel modules, Rust/Node toolchains, crate specs
src/                        CLI parsing and process/bootstrap composition
crates/                     42 top-level Rust product/support crates
crates/tools/               fine-grained built-in tool packages and registry
apps/desktop/               Electron Desktop application and its tests
tests/                      root architecture, process E2E, and regression tests
build_defs/                 shared Rust and web Bazel rules/macros
tools/                      version stamping and repository build utilities
docs/                       architecture and product documentation
install/                    release installation scripts
benchmarks/                 benchmark inputs and runners
design/                     design artifacts, not runtime source
```

Generated `bazel-*`, `node_modules`, coverage, and packaging outputs are never part of
the source layout.

## Root CLI and bootstrap

```text
src/
  main.rs                   executable composition root
  cli/
    args/                   clap argument groups
    commands/               Desktop and directory subcommands
    tests/                  parser and override tests
  bootstrap/
    hub/                    Hub discovery, spawn, registration, and uplink
    modes/                  TUI, server, ACP, attach, Hub, MetaHub, Desktop modes
    process/                parent liveness, housekeeping, startup protocol
    session/                resume and Worktree session preparation
    tests/                  bootstrap-focused tests
  logging.rs
  log_writer.rs
```

Root source is wiring, not a reusable domain layer. Reusable behavior belongs in a
crate; `src/main.rs` and `src/bootstrap` may depend on crates, while crates must never
depend on root `src/`.

## Rust crate families

The 42 top-level crate directories are grouped here by responsibility, not as a claim
that every crate in a row may depend on every other crate.

| Family | Crates |
| --- | --- |
| Contracts and APIs | `loopal-decision-api`, `loopal-error`, `loopal-provider-api`, `loopal-protocol`, `loopal-tool-api`, `loopal-vault-api` |
| Agent execution | `loopal-agent`, `loopal-agent-client`, `loopal-agent-hub`, `loopal-agent-server`, `loopal-classifier`, `loopal-context`, `loopal-kernel`, `loopal-runtime`, `loopal-scheduler`, `loopal-session`, `loopal-tool-invocation`, `loopal-turn` |
| Integrations and capabilities | `loopal-acp`, `loopal-backend`, `loopal-config`, `loopal-git`, `loopal-hooks`, `loopal-ipc`, `loopal-mcp`, `loopal-memory`, `loopal-meta-hub`, `loopal-prompt`, `loopal-prompt-system`, `loopal-provider`, `loopal-sandbox`, `loopal-storage`, `loopal-telemetry`, `loopal-workspace` |
| Secrets and vaults | `loopal-hub-vault`, `loopal-secret-client`, `loopal-secret-runtime`, `loopal-vault-age` |
| UI and verification | `loopal-tui`, `loopal-view-state`, `loopal-test-support`, `loopal-mock-llm` |

`crates/tools` further splits Agent, filesystem, and process tools into Bazel packages.
Its registry is shared infrastructure; individual tools depend on API contracts rather
than on the root binary or a UI.

The checked-in `BUILD.bazel` dependencies are authoritative. Preserve these direction
rules when changing them:

- API/protocol crates do not depend on concrete providers, UIs, or process adapters.
- Runtime/domain crates do not depend on `loopal-tui` or `apps/desktop`.
- UI and transport adapters depend inward on contracts and services.
- Root bootstrap is the Rust composition layer and may select concrete adapters.
- `loopal-test-support` and `loopal-mock-llm` are verification infrastructure, not
  production dependencies.

## Desktop application

```text
apps/desktop/
  src/
    base/common/             lifecycle, event, cancellation, async primitives
    shared/
      contracts/             cross-process schemas and the Desktop API facade
      i18n/                  English/Chinese message catalogs
      protocol/              Renderer/Main protocol constants
    platform/
      ipc/common/            environment-neutral channels and transports
      instantiation/common/  service identity/collection primitives
      desktop-host/
        common/              handshake contracts
        node/{host,process,rpc}/
      loopal-backend/
        common/{channels,clients}/
        node/{attention,backend,fake,federation,projections,runtime,
              sessions,settings,unavailable,workspace}/
    main/{app,media,sessions}/
    preload/                 narrow contextBridge adapter
    renderer/                Chromium entry and platform detection
    workbench/
      browser/               shell and application composition only
      contrib/{agents,attention,conversation,federation,session-panels,
               sessions,settings}/browser/
      services/{commands,contributions,layout,panes}/
  test/
    fixtures/workbench/      deterministic unit fixtures
    support/{backend,ipc,workbench}/
    staging/                 packaged-tree smoke test
  e2e/
    fake/                    deterministic Electron behavior specs
    real/host/{attention,federation}/
    real/provider/{attention,federation}/
    support/{electron,federation,fixtures,providers,runtime,settings}/
    fixtures/                copied workspace, LLM, MCP, plugin, and SSH data
```

Desktop dependency direction is deliberate:

```text
base/shared -> platform common -> platform node -> Electron main
shared + workbench services/browser/contrib -> renderer
shared + IPC/backend common -> preload
main + preload + renderer -> application composition, never reusable domain code
```

`shared` stays free of Electron, Node, and React. Common platform code stays
environment-neutral. Renderer/Workbench code must not import `main`, `preload`, or any
`platform/**/node` implementation. Main owns privileged OS access and concrete Host
wiring. Preload exposes only the validated `loopalDesktop` API.

`workbench/browser` owns shell state and composes first-party contributions. Feature UI,
tests, and CSS live with their `workbench/contrib` feature. Contributions may use shared
browser services and view models, but production feature code must not instantiate the
Workbench root.

## Verification ownership

- Crate unit/integration tests stay under their crate's `src/` or `tests/` Bazel target.
- Root `tests/architecture` enforces repository/process boundaries.
- Root `tests/e2e` covers Rust bootstrap, Hub lifecycle, and IPC behavior.
- Root `tests/regressions` locks down cross-crate failures.
- Desktop colocates component/unit tests with source and keeps reusable fixtures in
  `apps/desktop/test`.
- Desktop E2E placement and acceptance rules are defined in
  [the E2E contract](./desktop/e2e-contract.md).

## Change rules

1. Move tests and feature CSS with the production feature.
2. Update every relative import in the same change; do not leave compatibility shims in
   an old feature directory.
3. Do not create a new Bazel package casually: parent `glob()` calls stop traversing at
   nested `BUILD` boundaries. Add child targets and parent aggregation atomically.
4. Keep Electron entry paths stable unless `electron.vite.config.ts`, packaging, and
   staging tests change together.
5. Validate layout changes with the narrow Bazel typecheck/unit targets, then the E2E
   layer whose physical or runtime boundary changed.
