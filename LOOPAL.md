# LOOPAL.md

This file provides guidance to Loopal when working with code in this repository.

## Build & Test Commands

```bash
bazel build //:loopal                         # Build main binary
bazel build //...                             # Build everything
bazel test //...                              # Run all tests
bazel test //crates/loopal-tui:suite          # Run tests for a single crate
bazel build //... --config=clippy             # Clippy lint (must pass with zero warnings)
bazel build //... --config=rustfmt            # Rustfmt check
bazel build //:loopal -c opt                  # Optimized release build
CARGO_BAZEL_REPIN=1 bazel sync --only=crates  # Re-pin external crates after dependency changes
```

Convenience targets via Makefile: `make build`, `make test`, `make clippy`, `make fmt`, `make check` (clippy + fmt + test), `make install`.

## Architecture

Loopal is an AI coding agent with a TUI, built in Rust (edition 2024) using Bazel 8.1.0 as the build system. The codebase is organized as ~30 Rust crates in a layered architecture. Data flows top-down; each layer only depends on layers below it.

### Crate map

```
src/main.rs (bootstrap + CLI)
    ├─ loopal-tui            Terminal UI (ratatui). Event loop, input handling, views.
    ├─ loopal-runtime        Agent loop engine. Orchestrates: input → middleware → LLM → tools → repeat.
    ├─ loopal-kernel         Central registry. Owns tool/provider/hook registries + MCP manager.
    ├─ loopal-context        Context pipeline. Middleware chain for message compaction/budget.
    ├─ loopal-prompt         Prompt builder with fragment registry and template rendering.
    ├─ loopal-prompt-system  System prompt content and templates.
    ├─ loopal-provider       LLM providers (Anthropic, OpenAI, Google, OpenAI-compat). SSE streaming.
    ├─ loopal-agent          Sub-agent spawning, task store, inter-agent bridge.
    ├─ loopal-agent-hub      Hub coordinator: agent registry, UI dispatcher, event routing.
    ├─ loopal-agent-server   Agent server process. IPC frontend, session management.
    ├─ loopal-agent-client   Client for connecting to agent server via IPC.
    ├─ loopal-acp            Agent Client Protocol bridge for IDE integration (JSON-RPC).
    ├─ loopal-ipc            JSON-RPC 2.0 transport: stdio, TCP, duplex framing.
    ├─ loopal-session        Session controller, message log, rewind, state management.
    ├─ loopal-mcp            Model Context Protocol client. Spawns MCP servers, discovers tools.
    ├─ loopal-hooks          Pre/post tool-use lifecycle hooks executed as shell commands.
    ├─ loopal-scheduler      Cron-based task scheduling (expressions, tick, triggers).
    ├─ loopal-memory         Auto-memory observer and Memory tool.
    ├─ loopal-storage        Session + message persistence (~/.loopal/sessions/).
    ├─ loopal-config         5-layer config merge + Settings/HookConfig/SandboxConfig types.
    ├─ loopal-sandbox        Sandbox enforcement: command/path/network policy, platform adapters.
    ├─ loopal-backend        Filesystem, shell, network I/O backend (LocalBackend).
    ├─ loopal-git            Git operations: repo detection, worktree management, cleanup.
    ├─ loopal-provider-api   Provider/Middleware traits + ChatParams/StreamChunk/ModelInfo.
    ├─ loopal-tool-api       Tool trait + PermissionLevel/Mode/Decision + truncate_output.
    ├─ loopal-protocol       Envelope, AgentEvent, ControlCommand, AgentMode, AgentStatus.
    ├─ loopal-message        Message, ContentBlock, normalize_messages.
    ├─ loopal-error          LoopalError + all sub-error types.
    └─ loopal-test-support   Test harness, mock provider, fixtures, assertions.

tools/ (sub-workspace under crates/)
    ├─ registry/             Tool registry implementation.
    ├─ filesystem/           Read, Write, Edit, MultiEdit, ApplyPatch, Grep, Glob, Ls, Fetch, WebSearch, FileOps.
    ├─ process/              Bash, Background process management.
    └─ agent/                AskUser, PlanMode.
```

### Multi-process architecture (default)

```
TUI Process ──stdio IPC──→ Agent Server Process ←──TCP──→ IDE / CLI
                                    │
                              Agent Loop + Kernel
```

- TUI connects to Agent Server via stdio IPC (`loopal-agent-client`)
- Agent Server also opens a TCP listener for external clients (IDE, CLI)
- External clients discover the TCP port via `{tmp}/loopal/run/<pid>.json`
- ACP (`--acp` mode) bridges IDE's `session/*` protocol to Agent Server's `agent/*` IPC protocol

### Agent loop cycle (runtime)

`agent_loop/runner.rs` in `loopal-runtime`:
1. Wait for user input
2. Execute middleware pipeline (compaction, context guard)
3. Stream LLM response (text + tool calls)
4. Record assistant message
5. If tool calls: check permissions → parallel execute → loop
6. If no tool calls: wait for next input

### Extension points

- **New tool**: Implement `Tool` trait → register in tool registry (`crates/tools/registry/`)
- **New LLM provider**: Implement `Provider` trait → register in `kernel/provider_registry.rs`
- **New middleware**: Implement `Middleware` trait → add to pipeline in `bootstrap.rs`
- **MCP tools**: Configure `mcp_servers` in settings.json → auto-discovered at startup

## Configuration

```
~/.loopal/settings.json              Global settings
~/.loopal/LOOPAL.md                  Global instructions (injected into system prompt)
<project>/.loopal/settings.json      Project settings
<project>/.loopal/settings.local.json  Local overrides (gitignored)
```

Environment variable overrides use `LOOPAL_` prefix. Key settings: `model` (default: `claude-sonnet-4-20250514`), `max_turns` (default: 50), `permission_mode`.

## Code Conventions

- **200-line file limit** — all `.rs` files (including tests) must stay ≤200 lines. Split by SRP.
- Directory modules (`mod.rs` + submodules) are preferred over large single files.
- Inline `#[cfg(test)] mod tests` should be extracted to `tests/` when the file exceeds the limit.
- Test files are named `{feature}_test.rs` with edge cases in `{feature}_edge_test.rs`.
- Rust edition 2024. Standard Rust naming: `snake_case` for functions/variables, `PascalCase` for types, `SCREAMING_SNAKE_CASE` for constants.
- Comments and identifiers are in English.
- Architecture must conform to SOLID, GRASP, and YAGNI; balance cohesion and SRP — split by reason to change, not by line count.

## Permission System

Tools declare a `PermissionLevel` (ReadOnly / Supervised / Dangerous). The runtime's `PermissionMode` determines handling:
- `Bypass` — all tools auto-approved
- `AcceptEdits` — read-only auto-approved, writes need confirmation
- `Supervised` (default) — supervised/dangerous need user confirmation via TUI
- `Plan` — only read-only tools allowed

## Dependencies

External Rust dependencies are managed via `crate_universe` in `MODULE.bazel` using `crate.spec()` declarations. After adding or updating a dependency:

```bash
CARGO_BAZEL_REPIN=1 bazel sync --only=crates
```

Key dependencies: tokio (async runtime), ratatui/crossterm (TUI), reqwest (HTTP), clap (CLI), serde/serde_json (serialization), rmcp (MCP protocol), tracing (logging).

## Principles

- Architecture must conform to SOLID, GRASP, and YAGNI; files should stay under 200 lines; balance cohesion and SRP — split by reason to change, not by line count.
