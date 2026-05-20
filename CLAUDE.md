# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## ⛔ HARD RULE — NO OVER-COMMENTING

Code is the single source of truth. Before adding ANY comment, ask: does this line tell a fluent reader something they CANNOT see in the code itself? If no, do not write it.

**FORBIDDEN by default:**
- Module-level `//!` block comments that summarize what the module does
- `///` doc comments above pub fn / pub struct that restate the signature
- `///` doc comments above tests (the test name IS the doc)
- Field-level doc comments that just describe the field's name in English
- "Why we organize the file this way" / architecture narration in source files

**Allowed only when truly needed:**
- A short `// reason:` inline note when a line does something non-obvious that would otherwise look wrong (e.g. workaround for a bug, ordering constraint, performance hack)
- Public API doc when the contract has invariants or panics that aren't visible in types

When in doubt, delete. Re-deleting comments later is wasted effort.

## Build & Test Commands (Bazel)

```bash
bazel build //:loopal                         # Build main binary
bazel build //...                             # Build everything
bazel test //...                              # Run all tests
bazel test //crates/loopal-tui:suite          # Run tests for a single crate
bazel build //... --config=clippy             # Clippy lint (must pass with zero warnings)
bazel build //... --config=rustfmt            # Rustfmt check
bazel build //:loopal -c opt                  # Optimized release build
bazel build //:loopal -c opt --config=macos-arm  # Cross-compile for macOS ARM64
```

### Dependency management

External deps are managed via `crate_universe` from `Cargo.toml` / `Cargo.lock`.
After adding/updating a dependency in `Cargo.toml`:

```bash
CARGO_BAZEL_REPIN=1 bazel sync --only=crates   # Re-pin external crates
```

## Architecture

Loopal is an AI coding agent with a TUI, structured as 17 Rust crates in a layered architecture. Data flows top-down; each layer only depends on layers below it.

```
src/main.rs (bootstrap + CLI)
    ├─ loopal-tui          Terminal UI (ratatui). Event loop, input handling, views.
    ├─ loopal-runtime      Agent loop engine. Orchestrates: input → middleware → LLM → tools → repeat.
    ├─ loopal-kernel       Central registry. Owns tool/provider/hook registries + MCP manager.
    ├─ loopal-context      Context pipeline. Middleware chain for message compaction/limits.
    ├─ loopal-provider     LLM providers (Anthropic, OpenAI, Google, OpenAI-compat). SSE streaming.
    ├─ loopal-tools        Built-in tools (Read, Write, Edit, Bash, Grep, Glob, Ls, WebFetch).
    ├─ loopal-mcp          Model Context Protocol client. Spawns MCP servers, discovers tools.
    ├─ loopal-hooks        Pre/post tool-use lifecycle hooks executed as shell commands.
    ├─ loopal-storage      Session + message persistence (~/.loopal/sessions/).
    ├─ loopal-config       5-layer config merge + Settings/HookConfig/SandboxConfig types.
    ├─ loopal-provider-api Provider/Middleware traits + ChatParams/StreamChunk/ModelInfo.
    ├─ loopal-tool-api     Tool trait + PermissionLevel/Mode/Decision + truncate_output.
    ├─ loopal-protocol     Envelope, AgentEvent, ControlCommand, AgentMode, AgentStatus.
    ├─ loopal-message      Message, ContentBlock, normalize_messages.
    └─ loopal-error        LoopalError + all sub-error types (Provider/Tool/Config/Storage/Hook/Mcp).
```

### Key data flow

**Multi-process architecture (default):**

```
TUI Process ──stdio IPC──→ Agent Server Process ←──TCP──→ IDE / CLI
                                    │
                              Agent Loop + Kernel
```

- TUI connects to Agent Server via stdio IPC (`loopal-agent-client`)
- Agent Server also opens a TCP listener for external clients (IDE, CLI)
- External clients discover the TCP port via `{tmp}/loopal/run/<pid>.json`
- Multiple clients can join the same session (`agent/join`) or create independent sessions
- ACP (`--acp` mode) bridges IDE's `session/*` protocol to Agent Server's `agent/*` IPC protocol

**IPC protocol methods** (`agent/*` over JSON-RPC 2.0):
- Lifecycle: `initialize`, `agent/start`, `agent/shutdown`
- Data: `agent/message` (Envelope), `agent/control` (ControlCommand)
- Events: `agent/event` (notification), `agent/interrupt` (notification)
- Interactive: `agent/permission` (request/response), `agent/question` (request/response)
- Multi-client: `agent/join` (join existing session), `agent/list` (list sessions)

### Agent loop cycle (runtime)

`AgentLoopRunner::run()` in `agent_loop/runner.rs`:
1. Wait for user input
2. Execute middleware pipeline (compaction, context guard)
3. Stream LLM response (text + tool calls)
4. Record assistant message
5. If tool calls: check permissions → parallel execute → loop
6. If no tool calls: wait for next input

### Extension points

- **New tool**: Implement `Tool` (or `TypedTool<P>`) → register in `builtin/mod.rs`. **MUST** declare `secret_eligible_params()` (no default — forces explicit choice about secret exposure). Bash returns `&["command", "env"]`; everything else returns `&[]`. Write-tools also add `precheck` rejecting `<secret_ref:...>` via `loopal_secret_runtime::WIRE_REF_MARKER`.
- **New LLM provider**: Implement `Provider` trait → register in `kernel/provider_registry.rs`
- **New middleware**: Implement `Middleware` trait → add to pipeline in `bootstrap.rs`
- **MCP tools**: Configure `mcp_servers` in settings.json → auto-discovered at startup

## MCP startup model

MCP server spawn does not block `agent/start`. The Kernel holds a `Arc<dyn McpProvider>` strategy with two implementations:

- **`LocalMcpProvider`** (root agent): owns `Arc<RwLock<McpManager>>`. `spawn_background(configs)` fires the `start_all` future on a background `tokio::spawn` and returns immediately. `wait_until_settled(timeout)` races the background task against a deadline.
- **`McpProxyClient`** (sub-agent, depth > 0): forwards `list_tools` / `call_tool` / `snapshot` to root via `hub/mcp/*` IPC. Hub forwards those to `"main"` agent's `agent/mcp/*` handler, which calls the root's `LocalMcpProvider`. **Sub-agents do not spawn MCP processes** — they share root's connections.

`build_kernel_from_config` orchestrates:
1. `kernel.spawn_mcp()` — fire-and-forget (no-op for proxy)
2. `kernel.finalize_mcp_tools(LOOPAL_MCP_STARTUP_WAIT_SECS)` — bounded wait (default 5s), then register `McpToolAdapter` for every `(server, tool)` pair the provider reports
3. Slow servers settle in the background; subsequent reconnects register their tools via `kernel.register_mcp_tools_for_server(name)`

`McpConnection::connect` is wrapped in `tokio::time::timeout(config.timeout_ms)` defensively — bottoms out at the configured per-server limit even if the underlying rmcp transport ignores it.

## Vault + Secret runtime

Zero-trust secret management. LLM never sees plaintext. Architecture is split
into two layers: a **generic vault** (encrypted KV storage) and a **Loopal-specific
runtime** (LLM-facing placeholder + redaction).

### Layered architecture

```
loopal-vault-api      Vault trait + AuditSink trait + VaultError + VaultOp
    ↑                 (~80 lines; pure trait crate, depended on by all downstream)
loopal-vault-age      AgeVault impl + identity/recipients/editor + `loopal vault` CLI
    ↑                 (age+yaml backend; swappable for keychain/KMS in future)
loopal-secret-runtime template + resolver + redactor + hooks + JsonlAuditSink
    ↑                 (LLM-safety layer; depends only on vault-api trait)
loopal-config         build_secret_store: instantiates AgeVault with JsonlAuditSink
loopal-mcp/kernel/    inject Arc<dyn Vault>; expand_to_plaintext for spawn-time secrets
loopal-runtime        tool_pipeline hooks: apply_resolver + apply_redactor
```

### Data flow (three directions)

```
            ┌─────────────────────────────────────┐
            │  LLM (only sees <secret_ref:NAME>)  │
            └──▲────────────────────────────────▲─┘
               │ outbound                       │ tool_result (redacted)
   prompt assembly                              │
   ────────────────                             │
   {{secret:X}} → <secret_ref:X>                │
   loopal-secret-runtime::translate_outbound    │
                                                │
                                  ┌─────────────┴─────────────┐
                                  │  Redactor                 │
                                  │  plaintext → placeholder  │
                                  │  (BEFORE overflow-to-file)│
                                  └─────────────▲─────────────┘
                                                │
                              tool stdout (plaintext briefly here)
                                                │
                              ┌─────────────────┴─────────────────┐
                              │  Tool execute (Bash/Fetch/MCP)    │
                              │  ↑ plaintext only in child env    │
                              └─────────────────▲─────────────────┘
                                                │ resolved tool args
                              ┌─────────────────┴─────────────────┐
                              │  Resolver (whitelist only)        │
                              │  <secret_ref:X> → plaintext       │
                              └─────────────────▲─────────────────┘
                                                │ tool_use (placeholder)
                                                │ from LLM
```

### Components

- `loopal-vault-api::Vault` — async trait `get / list_names / put / delete / rekey`
- `loopal-vault-api::AuditSink` — trait the vault calls on every op; `VaultOp` enum covers Decrypted / Encrypted / Rekeyed / RecipientChanged
- `loopal-vault-age::AgeVault` — default per-vault impl (age + yaml + SSH identity + recipients); `loopal-vault-age::cli` is the `loopal vault [--name <name>]` (legacy `vault@<name>` accepted) + `loopal vaults` subcommands
- `loopal-secret-runtime::MergedVault` — composes multiple named vaults into a single flat `Vault` view (default-first + alphabetical, conflict warn)
- `loopal-secret-runtime::{template, resolver, redactor, hooks}` — placeholder syntax + tool argument substitution + output scrubbing
- `loopal-secret-runtime::JsonlAuditSink` — `impl AuditSink` writing `~/.loopal/telemetry/secret_access.jsonl` with mode 0600; also records runtime `Resolved` / `Redacted` events
- `loopal-config::build_secret_store` — instantiates `AgeVault::with_audit(..., JsonlAuditSink)` so vault ops and runtime hooks share one audit log
- `loopal-runtime::tool_pipeline` — calls `loopal_secret_runtime::apply_resolver` (Hook 2) before execute, `apply_redactor` (Hook 3) before overflow-to-file

### Per-tool checklist (adding a new tool)

`Tool::secret_eligible_params() -> &'static [&'static str]` has **no default**.
You must declare it explicitly. For typed tools (`TypedTool<P>`), the same method
is on the typed trait — the `TypedBridge` forwards it to `Tool` automatically,
so a missing declaration is a compile error in either path.

| Tool kind | Return |
|---|---|
| Reads only (Read/Glob/Grep/Ls/...) | `&[]` |
| Writes to user files (Write/Edit/MultiEdit/ApplyPatch) | `&[]` AND add `precheck` rejecting `<secret_ref:...>` (check string fields for `WIRE_REF_MARKER`) |
| Executes shell/network with secret-bearing args (Bash) | List of field names whose string values may legitimately contain `<secret_ref:NAME>`, e.g. `&["command", "env"]` |
| MCP tool adapter | `&[]` (MCP gets secrets via spawn-time env, not tool args) |

### Vault file layout

```
<project>/.loopal/vaults/
├── default.vault/                  # implicit default (init creates it)
│   ├── store.age                   # age-encrypted YAML (input: git ✓)
│   ├── recipients                  # SSH pubkeys, one per line (input: git ✓)
│   ├── .gitignore                  # auto-generated, excludes *.lock + *.tmp.*
│   ├── store.age.lock              # cross-process write lock (gitignored)
│   └── store.age.tmp.<pid>         # atomic-write tempfile (gitignored)
├── production.vault/               # additional vault, independent recipients/ACL
│   └── ...
└── personal.vault/                 # can be added to root .gitignore to keep local-only
    └── ...
```

CLI commands:

- **Vault set operations** (`loopal vaults <op>`):
  - `init [<name>]` — create a vault; name defaults to `default`
  - `list` — list all vaults (`*` marks the default)
  - `remove <name>` — delete a vault (forces `'rotated'` confirmation)
- **Single-vault operations** (`loopal vault [--name <name>] <op>`):
  - `vault <op>` (no `--name`) targets the default vault
  - `vault@<name> <op>` (legacy syntax, normalized to `--name <name>` before clap parsing)
  - Ops: `set <k> [--value <v>]` (stdin recommended), `get <k>`, `list`, `edit`, `rekey`, `recipients {add <pubkey> | remove <label> | list}`

Settings (all optional, `.loopal/settings.json`):
```json
{
  "secrets": {
    "vaults_dir": ".loopal/vaults",   // default; overrideable
    "default_vault": "default"        // default; e.g. "production"
  }
}
```

### Multi-vault and the merged view

LLM-bound code (system prompt, tool args) only sees a flat
`<secret_ref:NAME>` placeholder set. When multiple vaults exist, runtime
exposes a single `MergedVault` whose `list_names` is the union of all
vaults. Conflicts (same name in multiple vaults) resolve as:

1. The default vault wins.
2. Among non-default vaults, alphabetical order wins.

A `warn!` is emitted at startup for each shadowed key, naming the winner
and the shadowed vault. Writes (`put` / `delete`) target the default
vault; vault-specific operations remain available through the CLI.

### Threat model

**Protected against**:
- Disk theft / lost laptop (vault is age-encrypted)
- Accidental `git add` of plaintext (`{{secret:X}}` author syntax is harmless placeholder)
- LLM provider seeing plaintext (LLM only ever sees `<secret_ref:X>` placeholders)
- Session persistence leakage (`~/.loopal/sessions/` stores placeholders, never plaintext)
- Tool stdout/stderr echoing plaintext (Redactor scrubs before tool_result returns to LLM)
- Concurrent vault writes (cross-process `.lock` file)
- Tracing logs leaking plaintext (sentinel test in `loopal-runtime/tests/suite/tracing_sentinel_test.rs`)

**NOT protected against**:
- Compromised SSH private key (whoever has the key has the vault)
- Plaintext lifetime in tool child process memory (Bash → `curl` → token in `curl`'s heap)
- `ps`-visibility of `Bash.command` field when secret is substituted there (audit emits warn — prefer `env` field)
- Removed git recipients who already cloned the repo (must rotate values at provider side after `vaults remove <name>` or `vault --name <name> recipients remove`)
- LLM prompt-injection writing secrets it learns to plaintext stdout (LLM never has plaintext to inject; redactor scrubs known plaintext)
- Memory dumps / swap files / core dumps containing plaintext during the brief window in resolver cache

### Author vs wire placeholder syntax

- **Author syntax** `{{secret:NAME}}` — used in `<project>/.loopal/memory/`, `LOOPAL.md`, `settings.json` field values, provider `api_key`, MCP `env`/`headers`/`url`. Translated to wire form (or expanded to plaintext for provider/MCP) before LLM or subprocess sees anything.
- **Wire syntax** `<secret_ref:NAME>` — what the LLM sees in system prompt and writes back in tool_use arguments. Resolver substitutes plaintext only for fields listed in `secret_eligible_params`. Redactor scrubs known plaintext back to wire form before tool_result returns to LLM.

Both syntaxes use the strict regex `[a-z][a-z0-9_]*` for NAME.

## Configuration

```
~/.loopal/settings.json          Global settings
~/.loopal/LOOPAL.md              Global instructions (injected into system prompt)
~/.loopal/classifier.md          Optional custom Classifier-mode system prompt
<project>/.loopal/settings.json  Project settings
<project>/.loopal/classifier.md  Project-level Classifier prompt override
<project>/.loopal/settings.local.json  Local overrides (gitignored)
```

`classifier.md` is loaded in the same global → project → local order as settings, but with **replace semantics** (highest-priority non-empty layer wins; not concatenated). Absent on every layer ⇒ the built-in default prompt is used.

Environment variable overrides use `LOOPAL_` prefix. Key settings:
- `LOOPAL_MODEL` — default model id (default: `claude-opus-4-7`)
- `LOOPAL_PERMISSION_MODE` — `bypass` / `ask_dangerous` / `ask_any_write`
- `LOOPAL_DECISION_MODE` — `manual` / `classifier` / `agent`
- `LOOPAL_CLASSIFIER_TIMEOUT_SECS` — Classifier LLM timeout (default 180s)
- `LOOPAL_TELEMETRY_DIR` — override telemetry dir (default `~/.loopal/telemetry/`); JSONL files like `classifier_outraced.jsonl` are written here
- `LOOPAL_SANDBOX` — sandbox policy

## Code Conventions

- **200-line file limit** — all `.rs` files (including tests) must stay ≤200 lines. Split by SRP.
- Directory modules (`mod.rs` + submodules) are preferred over large single files.
- Inline `#[cfg(test)] mod tests` should be extracted to `tests/` when the file exceeds the limit.
- Test files are named `{feature}_test.rs` with edge cases in `{feature}_edge_test.rs`.
- Comments and identifiers follow the language of existing code in each file.

## Permission System

Permission is decomposed into two orthogonal dimensions:

**PermissionMode** (when to ask): `bypass` (default) / `ask_dangerous` / `ask_any_write`
**DecisionMode** (who answers): `manual` (default) / `classifier` / `agent`

Tools declare a `PermissionLevel` (`ReadOnly` / `Write` / `Dangerous`). `PermissionMode::check(level)` returns `Allow` / `Ask` / `Deny`; `Ask` outcomes are dispatched to the handler chain selected by `DecisionMode` at session setup.

`Classifier` mode runs a single LLM call (≤180s) against `classifier.md` (or built-in default) and races the user. `Agent` mode is reserved for a future sub-agent implementation; today the factory transparently falls back to `Classifier` with a warning. For `AskUser` questions in Classifier mode, the classifier may **abstain** (empty inner array) on subjective preferences, which transparently defers the question to the user.

## Principles

- Architecture must conform to SOLID, GRASP, and YAGNI; files should stay under 200 lines; balance cohesion and SRP — split by reason to change, not by line count.
- Names must be specific and descriptive — files, modules, functions, and variables should say exactly what they do. Avoid vague names like `common`, `helpers`, `utils`, `misc`, `edge_test`, `manager`, `handler`, `data`, `info`, `process`.
- Code is the single source of truth (SSOT) — do not over-comment. Comments explain *why* (non-obvious decisions, constraints, invariants), never *what*. Delete comments that paraphrase the next line, restate function signatures, narrate steps, or describe what a well-named test/function already conveys. Test function names ARE the documentation — no `///` doc above them. Inline `// reason:` lines are only justified when the reason is not derivable from reading the code.
- After completing a task, verify that unit and integration test coverage for all changed code is ≥ 95%. Audit every new/modified file, identify untested code paths, and add missing tests before considering the task done.
