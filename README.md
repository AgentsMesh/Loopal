<p align="center">
  <pre align="center">
  _        ___    ___   ____     _     _
 | |      / _ \  / _ \ |  _ \   / \   | |
 | |     | | | || | | || |_) | / _ \  | |
 | |___  | |_| || |_| ||  __/ / ___ \ | |___
 |_____|  \___/  \___/ |_|   /_/   \_\|_____|
  </pre>
  <strong>An agentic AI coding tool that lives in your terminal.</strong><br/>
  Built in Rust. Multi-model. Multi-agent. Extensible.<br/><br/>
  Part of <a href="https://agentsmesh.ai">AgentsMesh.ai</a>
</p>

<p align="center">
  <a href="https://youtube.com/shorts/Lptchj75HP8">
    <img src="https://img.youtube.com/vi/Lptchj75HP8/maxresdefault.jpg" alt="Loopal Demo" width="600"/>
  </a>
  <br/>
  <em>Click to watch the demo</em>
</p>

<p align="center">
  <a href="#installation">Installation</a> &bull;
  <a href="#quick-start">Quick Start</a> &bull;
  <a href="#features">Features</a> &bull;
  <a href="#configuration">Configuration</a> &bull;
  <a href="#architecture">Architecture</a> &bull;
  <a href="#license">License</a>
</p>

---

## Installation

### From GitHub Releases (recommended)

Download pre-built binaries from [Releases](https://github.com/AgentsMesh/Loopal/releases):

| Platform | Target |
|---|---|
| macOS (Apple Silicon) | `aarch64-apple-darwin` |
| Linux (x86_64) | `x86_64-unknown-linux-gnu` |
| Linux (ARM64) | `aarch64-unknown-linux-gnu` |
| Windows (x86_64) | `x86_64-pc-windows-msvc` |

### Build from source

```bash
git clone https://github.com/AgentsMesh/Loopal.git && cd Loopal
make install   # builds optimized binary → ~/.local/bin/loopal
```

Requires [Bazel 8+](https://bazel.build/) (via [Bazelisk](https://github.com/bazelbuild/bazelisk)).

## Quick Start

```bash
# Set your API key
export ANTHROPIC_API_KEY="sk-..."   # or OPENAI_API_KEY, GOOGLE_API_KEY

# Start Loopal in your project
cd your-project && loopal

# Or pass a prompt directly
loopal "explain the architecture of this project"
```

## Features

### Multi-Provider LLM Support

Works out of the box with **Anthropic**, **OpenAI**, **Google**, and any **OpenAI-compatible** endpoint. Switch models on the fly with `-m`:

```bash
loopal -m claude-sonnet-4-20250514
loopal -m gpt-4o
loopal -m gemini-2.5-pro
```

Supports thinking/reasoning modes (auto, effort levels, token budgets) and per-task model routing.

### Rich Terminal UI

Full-featured interactive TUI built with [Ratatui](https://ratatui.rs):

- Markdown rendering with syntax highlighting
- Streaming responses with real-time progress
- Permission approval dialogs
- Multi-agent topology visualization
- Plan/Act mode toggle
- Slash-command completion
- Session resume (`loopal -r <session-id>`)

### 17 Built-in Tools

| Category | Tools |
|---|---|
| **File I/O** | Read, Write, Edit, MultiEdit, ApplyPatch, CopyFile, MoveFile, Delete |
| **Search** | Grep (regex, context, file filters), Glob (pattern matching), Ls |
| **Process** | Bash (foreground + background), Fetch, WebSearch |
| **Agent** | AskUser, EnterPlanMode, ExitPlanMode |

All tools go through sandbox policy checks before execution.

### Multi-Agent Orchestration

Spawn sub-agents that run in parallel, communicate via message passing, and coordinate through a shared task store. The TUI provides a topology view to observe and interact with any agent in the tree.

### MCP Integration

First-class [Model Context Protocol](https://modelcontextprotocol.io/) support:

- **Stdio** transport — spawn MCP servers as child processes
- **Streamable HTTP** transport — connect to remote MCP servers
- **OAuth** — automatic browser-based auth for protected servers

```json
{
  "mcp_servers": {
    "my-server": {
      "command": "npx",
      "args": ["-y", "@my/mcp-server"]
    }
  }
}
```

### IDE Integration (ACP)

Run as an [Agent Client Protocol](https://agentclientprotocol.com/) server for IDE integration:

```bash
loopal --acp   # JSON-RPC 2.0 over stdin/stdout
```

Works with Zed, JetBrains, Neovim, and any ACP-compatible editor.

### Skills & Memory

- **Skills** — Extend agent capabilities with project-specific markdown skill files (`/skill-name` invocation)
- **Memory** — Cross-session persistent memory that remembers observations and preferences

### Sandbox & Permissions

Three permission modes to control what the agent can do:

| Mode | Behavior |
|---|---|
| `bypass` | Auto-approve everything |
| `auto` | Smart approval based on intent classification |
| `supervised` | Require user confirmation for writes and commands |

Sandbox policies (strict/permissive/disabled) enforce filesystem, network, and command restrictions.

### Lifecycle Hooks

Run custom scripts on agent events — tool calls, session start, permission requests, etc:

```json
{
  "hooks": [
    {
      "event": "tool_call_post",
      "tool_filter": ["Bash"],
      "command": "notify-send 'Command executed'"
    }
  ]
}
```

## Configuration

Loopal uses a layered config system — each layer overrides the previous:

```
~/.loopal/settings.json              # Global settings
~/.loopal/LOOPAL.md                  # Global system prompt instructions
<project>/.loopal/settings.json      # Project settings
<project>/.loopal/settings.local.json  # Local overrides (gitignored)
<project>/LOOPAL.md                  # Project instructions
```

Key settings:

```json
{
  "model": "claude-sonnet-4-20250514",
  "permission_mode": "supervised",
  "thinking": { "type": "auto" },
  "providers": {
    "anthropic": { "api_key": "..." }
  },
  "mcp_servers": { },
  "sandbox": { "policy": "strict" }
}
```

Environment variables: `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `GOOGLE_API_KEY`.

## Architecture

Loopal runs as a multi-process, Hub-centric system. **Hub** is the sole coordinator — all agents and UI clients connect to it, no direct agent-to-agent communication.

```
                    ┌───────────┐   ┌───────────┐   ┌───────────┐
                    │    TUI    │   │ ACP (IDE) │   │    CLI    │
                    └─────┬─────┘   └─────┬─────┘   └─────┬─────┘
                          │   TCP (JSON-RPC 2.0)          │
                          └───────────┬───────────────────┘
                                      │
┌─────────────────────────────────────▼──────────────────────────────┐
│                              Hub                                   │
│                                                                    │
│   Registry ── agent parent/child tree + lifecycle tracking         │
│   Dispatch ── broadcast events to all connected UI clients         │
│   Relay    ── race permission/question requests to UIs             │
│   Spawner  ── fork agent processes + bridge completion results     │
│                                                                    │
│               stdio (JSON-RPC 2.0)                                 │
│              ┌────────────┼────────────┐                           │
│              │            │            │                            │
│   ┌──────────▼──┐  ┌─────▼──────┐  ┌──▼───────────┐              │
│   │ Root Agent  │  │ Sub-Agent  │  │ Sub-Agent ..  │              │
│   │             │  │            │  │               │              │
│   │ Kernel      │  │ Kernel     │  │ Kernel        │              │
│   │ LLM Stream  │  │ LLM Stream │  │ LLM Stream    │              │
│   │ Tools       │  │ Tools      │  │ Tools         │              │
│   │ MCP Servers │  │ MCP Servers│  │ MCP Servers   │              │
│   └─────────────┘  └────────────┘  └───────────────┘              │
└────────────────────────────────────────────────────────────────────┘
```

**How it works:**

- **Agents** connect to Hub via stdio pipes (forked child processes). Each agent runs its own Kernel with LLM providers, tools, and MCP servers.
- **UI clients** (TUI, IDE via ACP, CLI) connect via TCP. Multiple clients can observe and interact with the same session simultaneously.
- **Sub-agents** are spawned on demand — Hub forks a new process, registers the parent/child relationship, and bridges completion results back to the parent as normal messages.
- **Events** are broadcast to all UI clients. **Permissions** are raced to all connected UIs — first response wins.
- All communication uses JSON-RPC 2.0, whether over stdio or TCP.

Built as 40+ Rust crates in a layered architecture — see [CLAUDE.md](./CLAUDE.md) for the full dependency graph and development guide.

## CLI Reference

```
Usage: loopal [OPTIONS] [PROMPT]...

Arguments:
  [PROMPT]...               Initial prompt

Options:
  -m, --model <MODEL>       Model to use
  -r, --resume <SESSION>    Resume a previous session
  -P, --permission <MODE>   Permission mode (bypass/auto/supervised)
      --plan                Start in plan mode (read-only)
      --headless            Process prompt and exit (no TUI)
      --worktree            Create isolated git worktree
      --no-sandbox          Disable sandbox enforcement
      --acp                 Run as ACP server
  -h, --help                Print help
```

## License

Proprietary. Copyright (c) 2024-2026 AgentsMesh.ai. All Rights Reserved.

See [LICENSE](./LICENSE) for full terms.
