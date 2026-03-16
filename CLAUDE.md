# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

```bash
cargo check --workspace          # Fast compilation check
cargo build                      # Debug build
cargo test --workspace           # Run all tests
cargo test -p loopagent-tui      # Run tests for a single crate
cargo test -p loopagent-tui --test app_test  # Run a single test file
cargo clippy --workspace --tests # Lint (must pass with zero warnings)
LOOPAGENT_LOG=debug cargo run    # Run with debug logging
```

## Architecture

LoopAgent is an AI coding agent with a TUI, structured as 11 Rust crates in a layered architecture. Data flows top-down; each layer only depends on layers below it.

```
src/main.rs (bootstrap + CLI)
    ├─ loopagent-tui          Terminal UI (ratatui). Event loop, input handling, views.
    ├─ loopagent-runtime      Agent loop engine. Orchestrates: input → middleware → LLM → tools → repeat.
    ├─ loopagent-kernel       Central registry. Owns tool/provider/hook registries + MCP manager.
    ├─ loopagent-context      Context pipeline. Middleware chain for message compaction/limits.
    ├─ loopagent-provider     LLM providers (Anthropic, OpenAI, Google, OpenAI-compat). SSE streaming.
    ├─ loopagent-tools        Built-in tools (Read, Write, Edit, Bash, Grep, Glob, Ls, WebFetch).
    ├─ loopagent-mcp          Model Context Protocol client. Spawns MCP servers, discovers tools.
    ├─ loopagent-hooks        Pre/post tool-use lifecycle hooks executed as shell commands.
    ├─ loopagent-storage      Session + message persistence (~/.loopagent/sessions/).
    ├─ loopagent-config       5-layer config merge: defaults → global → project → local → env vars.
    └─ loopagent-types        Shared types: Message, AgentEvent, UserCommand, Tool/Provider/Middleware traits.
```

### Key data flow

Three async channels connect TUI ↔ runtime:
- `AgentEvent` (256-cap): runtime → TUI (stream text, tool calls, token usage, etc.)
- `UserCommand` (16-cap): TUI → runtime (messages, mode switch, clear, model switch)
- `bool` (16-cap): TUI → runtime (permission approve/deny for tool execution)

The TUI has an **Inbox queue** (`VecDeque<String>`) that buffers user messages when the agent is busy. Messages auto-forward when the agent becomes idle (`AwaitingInput` event).

### Agent loop cycle (runtime)

`AgentLoopRunner::run()` in `agent_loop/runner.rs`:
1. Wait for user input
2. Execute middleware pipeline (compaction, context guard)
3. Stream LLM response (text + tool calls)
4. Record assistant message
5. If tool calls: check permissions → parallel execute → loop
6. If no tool calls: wait for next input

### Extension points

- **New tool**: Implement `Tool` trait → register in `builtin/mod.rs`
- **New LLM provider**: Implement `Provider` trait → register in `kernel/provider_registry.rs`
- **New middleware**: Implement `Middleware` trait → add to pipeline in `bootstrap.rs`
- **MCP tools**: Configure `mcp_servers` in settings.json → auto-discovered at startup

## Configuration

```
~/.loopagent/settings.json          Global settings
~/.loopagent/LOOPAGENT.md           Global instructions (injected into system prompt)
<project>/.loopagent/settings.json  Project settings
<project>/.loopagent/settings.local.json  Local overrides (gitignored)
```

Environment variable overrides use `LOOPAGENT_` prefix. Key settings: `model` (default: `claude-sonnet-4-20250514`), `max_turns` (default: 50), `permission_mode`.

## Code Conventions

- **200-line file limit** — all `.rs` files (including tests) must stay ≤200 lines. Split by SRP.
- Directory modules (`mod.rs` + submodules) are preferred over large single files.
- Inline `#[cfg(test)] mod tests` should be extracted to `tests/` when the file exceeds the limit.
- Test files are named `{feature}_test.rs` with edge cases in `{feature}_edge_test.rs`.
- Comments and identifiers follow the language of existing code in each file.

## Permission System

Tools declare a `PermissionLevel` (ReadOnly / Supervised / Dangerous). The runtime's `PermissionMode` determines handling:
- `BypassPermissions` — all tools auto-approved
- `AcceptEdits` — read-only auto-approved, writes need confirmation
- `Default` — supervised/dangerous need user confirmation via TUI
- `Plan` — only read-only tools allowed

## Principles

- Architecture must conform to SOLID, GRASP, and YAGNI; files should stay under 200 lines; balance cohesion and SRP — split by reason to change, not by line count.
