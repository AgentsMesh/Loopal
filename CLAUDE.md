# CLAUDE.md

Contributor guidance for agents working in this repository. Product behavior lives in
code and tests; architecture and layout facts belong in the linked documentation.

## Hard rule: do not over-comment

Before adding a comment, ask whether it tells a fluent reader something that cannot be
seen in the code. If not, omit it.

Forbidden by default:

- module/doc comments that paraphrase a module, signature, field, or test name;
- architecture narration in source files;
- step-by-step comments above self-explanatory code.

A short `// reason:` is appropriate for a non-obvious constraint, workaround, ordering
requirement, or safety invariant. Public API documentation is appropriate when the
contract, panic, or invariant is not visible in types.

## Build and test: Bazel only

```bash
bazel build //:loopal                         # main binary
bazel build //...                             # all build targets
bazel test //...                              # all tests
bazel test //crates/loopal-tui:suite          # one crate suite
bazel build //... --config=clippy             # zero-warning Clippy
bazel build //... --config=rustfmt            # formatting check
bazel build //:loopal -c opt                  # optimized binary
bazel build //:loopal -c opt --config=macos-arm
```

Desktop commands are indexed in
[docs/desktop/testing.md](./docs/desktop/testing.md) and
[docs/desktop/build-and-release.md](./docs/desktop/build-and-release.md). Do not invoke
Vite, Vitest, Playwright, electron-builder, npm scripts, or Cargo directly.

### Dependency management

There is no Cargo workspace and no checked-in `Cargo.toml` / `Cargo.lock`. External Rust
crates are declared with `crate.spec()` in `MODULE.bazel`; Bazel records resolution in
`MODULE.bazel.lock`.

```bash
bazel sync --only=crates
bazel build //...
```

JavaScript dependencies use `package.json` and `pnpm-lock.yaml`, imported by Bazel's npm
extension. pnpm may update the lockfile; Bazel still owns every build/test action.

## Architecture and repository boundaries

Loopal contains 42 top-level Rust crates, additional fine-grained tool packages under
`crates/tools`, and the Electron application under `apps/desktop`.

- [Repository layout](./docs/repository-layout.md) is the source of truth for directories
  and allowed dependencies.
- [Architecture principles](./docs/architecture.md) own Hub/Agent resource decisions.
- [Desktop documentation](./docs/desktop/README.md) owns Electron runtime, experience,
  verification, and release contracts.

```text
src/main.rs
  -> src/cli + src/bootstrap
  -> Hub / MetaHub / Agent process adapters
  -> runtime, kernel, provider, tools, Session, storage, and protocol crates

TUI / ACP / Desktop
  -> typed Hub and Agent protocols
  -> root and child Agent processes
  -> input -> context -> LLM -> tools -> persistence -> next turn
```

Dependency direction is inward: API/protocol crates know no concrete UI or provider;
runtime/domain crates know no TUI or Electron; transports and UIs depend on contracts;
root bootstrap selects concrete adapters. Root `src/` is composition, not a library.

### Extension points

- Tool: implement `loopal_tool_api::Tool`, place it in the matching `crates/tools` or
  Agent tool family, and register it through tool assembly.
- Provider: implement `loopal_provider_api::Provider` under `loopal-provider` and
  register it in `loopal-kernel`.
- Context stage: put reusable behavior in `loopal-context` and wire it from runtime.
- MCP: extend the Hub-owned/proxied boundary; never add a second subprocess owner.
- Desktop: add typed shared contracts first, privileged platform/Main behavior second,
  and UI/tests/CSS in the matching `workbench/contrib` feature.

## MCP startup contract

MCP startup never blocks `agent/start`. The root strategy starts configured connections
in the background and performs only a bounded initial wait. Slow servers settle later
and register tools dynamically. Subagents use `McpProxyClient`; they do not spawn their
own MCP processes. Every connection attempt is bounded by its configured timeout.

## Vault and secret safety

The LLM and persisted Session data see placeholders, never plaintext:

```text
loopal-vault-api -> loopal-vault-age
loopal-secret-runtime -> loopal-config / MCP / kernel / runtime
```

- Author syntax is `{{secret:NAME}}`; wire syntax is `<secret_ref:NAME>`.
- Names match `[a-z][a-z0-9_]*`.
- Resolve plaintext only at an actual consumer and keep it in `Zeroizing<String>`.
- Redact before overflow-to-file, persistence, tool-result return, or logging.
- Never copy plaintext into Renderer state, protocol journals, or snapshots.
- Vault operations and runtime resolution/redaction share the protected audit log.

Every `Tool`/`TypedTool` must declare `secret_eligible_params()` explicitly:

| Tool kind | Contract |
| --- | --- |
| Read-only | `&[]` |
| Writes user files | `&[]` plus precheck rejecting `WIRE_REF_MARKER` |
| Shell/network consumer | only fields that legitimately accept placeholders |
| MCP adapter | `&[]`; MCP secrets enter through its owned configuration boundary |

Detailed ownership and threat decisions are in
[architecture principles](./docs/architecture.md) and
[Stage 1 vault design](./docs/stage1-vault-on-hub.md).

## Configuration

```text
~/.loopal/settings.json                  global settings
~/.loopal/LOOPAL.md                      global instructions
~/.loopal/classifier.md                  optional global classifier prompt
~/.loopal/sessions/{id}/memory.db        per-Session derived memory index
<project>/.loopal/settings.json          project settings
<project>/.loopal/settings.local.json    local, gitignored overrides
<project>/.loopal/classifier.md          project classifier override
<project>/.loopal/memory/*.md            user source-of-truth memory notes
```

Never write derived `memory.db` data under `.loopal/memory`. Classifier prompts use
highest-priority replace semantics. Environment overrides use the `LOOPAL_` prefix;
common keys include `LOOPAL_MODEL`, `LOOPAL_PERMISSION_MODE`, `LOOPAL_DECISION_MODE`,
`LOOPAL_CLASSIFIER_TIMEOUT_SECS`, `LOOPAL_TELEMETRY_DIR`, and `LOOPAL_SANDBOX`.

## Code and test conventions

- Handwritten production and test files should stay at or below 200 lines; split by
  reason to change, not by arbitrary line chunks.
- Prefer directory modules and descriptive feature names over `helpers`, `misc`,
  `manager`, `handler`, or `data`.
- Move tests and feature assets with their production feature.
- Use `rg` for search and `apply_patch` for content edits.
- Preserve unrelated changes in a dirty worktree.
- Add explicit tests for new failure, cancellation, shutdown, generation, and security
  paths; changed code should maintain at least 95% relevant coverage.
- Before handoff, run the narrow Bazel targets and `git diff --check`.

## Permission model

Permission has two independent dimensions:

- `PermissionMode`: `bypass`, `ask_dangerous`, or `ask_any_write` decides when to ask.
- `DecisionMode`: `manual`, `classifier`, or `agent` decides who answers.

Tools declare `ReadOnly`, `Write`, or `Dangerous`. Classifier mode races a bounded model
decision with the user and may abstain on subjective questions. Agent mode currently
falls back to Classifier with a warning; do not document it as an implemented separate
decision agent.

## Design principles

Follow SOLID, GRASP, YAGNI, and least authority. Code is the behavioral source of truth;
tests are the executable contract; documents own only cross-cutting decisions and maps.
Use specific names, explicit process ownership, bounded resources, typed protocols, and
generation-safe state. Do not solve a missing workflow by exposing a generic filesystem,
shell, transport, or secret primitive.
