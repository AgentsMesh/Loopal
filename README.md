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
  <a href="#distributed-cluster">Distributed Cluster</a> &bull;
  <a href="#configuration">Configuration</a> &bull;
  <a href="#architecture">Architecture</a> &bull;
  <a href="#license">License</a>
</p>

---

## Installation

### Quick Install (recommended)

```bash
curl -fsSL https://raw.githubusercontent.com/AgentsMesh/Loopal/main/install/install.sh | bash
```

Options:

```bash
# Install to a custom directory
INSTALL_DIR=/usr/local/bin curl -fsSL https://raw.githubusercontent.com/AgentsMesh/Loopal/main/install/install.sh | bash

# Install a specific version
VERSION=0.1.1 curl -fsSL https://raw.githubusercontent.com/AgentsMesh/Loopal/main/install/install.sh | bash
```

Supports macOS (Apple Silicon) and Linux (x86_64 / ARM64). Installs to `~/.local/bin` by default.

### From GitHub Releases

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

# Server mode (no TUI, for CI/scripting)
loopal --server --ephemeral "run all tests and fix failures"
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
- Image attachment via clipboard paste (Ctrl+V)
- Inline diff visualization for code changes
- Real-time thinking process display with token count
- Permission approval dialogs
- Multi-agent topology visualization
- Status dashboard (session info, config, token usage)
- Background task panel
- Rewind to any previous turn
- Plan/Act mode toggle
- Slash-command completion
- Session resume (`loopal -r <session-id>`)

### 27 Built-in Tools

| Category | Tools |
|---|---|
| **File I/O** | Read, Write, Edit, MultiEdit, ApplyPatch, CopyFile, MoveFile, Delete |
| **Search** | Grep (regex, context, file filters), Glob (pattern matching), Ls |
| **Process** | Bash (foreground + background), Fetch |
| **Interaction** | AskUser, EnterPlanMode, ExitPlanMode |
| **Orchestration** | Agent (spawn sub-agents), SendMessage, ListHubs |
| **Task Management** | TaskCreate, TaskUpdate, TaskList, TaskGet |
| **Scheduling** | CronCreate, CronDelete, CronList |
| **Knowledge** | Memory (cross-session persistent observations) |

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

### Skills

Extend agent capabilities with project-specific markdown skill files. Place `.md` files in `.loopal/skills/` and invoke them as `/skill-name`:

```markdown
---
description: Create a git commit
---

Analyze all staged changes and draft a commit message.
User notes: $ARGUMENTS
```

- Skill name = filename stem (e.g., `commit.md` → `/commit`)
- `$ARGUMENTS` placeholder receives user input after the command
- Skills merge across config layers — project skills override global ones

### Memory

Cross-session persistent memory stored at `.loopal/memory/MEMORY.md`. The agent reads and writes this file to remember observations, preferences, and project context across conversations.

- Initialize with `/init` command
- Agent records observations via the built-in `Memory` tool
- LLM-based consolidation keeps memory concise and up-to-date

### Context Management

Automatic context window management with a 4-layer degradation pipeline:

| Layer | Trigger | Action |
|---|---|---|
| 0 | Always | Strip old thinking blocks and ephemeral content |
| 1 | >60% budget | Truncate oversized old tool results |
| 2 | >75% budget | LLM-based summarization of older messages |
| 3 | >90% budget | Emergency compaction — summarize + drop oldest |

Use `/compact` to trigger compaction manually.

### Built-in Commands

| Command | Description |
|---|---|
| `/plan` | Switch to plan mode (read-only) |
| `/act` | Switch to act mode (execution enabled) |
| `/model [name]` | Switch model or open model picker |
| `/compact` | Compact conversation context |
| `/rewind` | Rewind to a previous turn |
| `/status` | Show session info, config, and token usage |
| `/resume [id]` | Resume a previous session |
| `/init` | Initialize project config and memory |
| `/agents` | Show sub-agent status |
| `/topology` | Toggle agent topology overlay |
| `/clear` | Clear conversation history |

### Sandbox & Permissions

Three permission modes to control what the agent can do:

| Mode | Behavior |
|---|---|
| `bypass` | Auto-approve everything |
| `auto` | Smart approval based on intent classification |
| `supervised` | Require user confirmation for writes and commands |

Sandbox policies control filesystem and command restrictions:

| Policy | Behavior |
|---|---|
| `default-write` | OS sandbox allows all writes; app-level path_checker guards sensitive files (default) |
| `read-only` | All writes blocked, reads only |
| `disabled` | No sandbox enforcement |

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

## Distributed Cluster

Connect multiple Loopal instances into a cluster via **MetaHub** — a lightweight TCP coordinator that enables cross-hub agent communication.

### Start a cluster

```bash
# Terminal 1: Start MetaHub coordinator
loopal --meta-hub 0.0.0.0:9900

# Terminal 2: Connect first Hub
LOOPAL_META_HUB_TOKEN=<token> loopal --join-hub 127.0.0.1:9900 --hub-name code-hub

# Terminal 3: Connect second Hub
LOOPAL_META_HUB_TOKEN=<token> loopal --join-hub 127.0.0.1:9900 --hub-name review-hub
```

### What agents can do in a cluster

- **Discover hubs** — `ListHubs` tool shows all connected hubs and their agents
- **Spawn remotely** — `Agent` tool with `target_hub` parameter creates agents on other hubs
- **Route messages** — `SendMessage` to `"hub-name/agent-name"` routes through MetaHub
- **Auto-relay** — Permissions and events automatically propagate across hubs

### Three orthogonal execution dimensions

| Dimension | Options | Controls |
|---|---|---|
| **Frontend** | `--server` / TUI (default) | Who sees events and approves actions |
| **Lifecycle** | `--ephemeral` / persistent (default) | Exit on idle vs. wait for next task |
| **Cluster** | `--join-hub` / standalone (default) | Single instance vs. distributed |

All combinations are valid — a `--server --join-hub` instance is a headless cluster worker.

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
  "sandbox": { "policy": "default-write" }
}
```

Environment variables: `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `GOOGLE_API_KEY`.

## Architecture

Loopal runs as a multi-process, Hub-centric system. Multiple Hubs can form a cluster via **MetaHub**.

### Single Hub (default)

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
│   │ Kernel+LLM  │  │ Kernel+LLM │  │ Kernel+LLM   │              │
│   │ Tools+MCP   │  │ Tools+MCP  │  │ Tools+MCP     │              │
│   └─────────────┘  └────────────┘  └───────────────┘              │
└────────────────────────────────────────────────────────────────────┘
```

### Distributed Cluster (via MetaHub)

```
                         ┌──────────────┐
                         │   MetaHub    │
                         │  (TCP coord) │
                         └──┬───────┬───┘
                    TCP     │       │     TCP
               ┌────────────┘       └────────────┐
               │                                  │
        ┌──────▼──────┐                   ┌───────▼─────┐
        │   Hub-A     │                   │   Hub-B     │
        │  (uplink)   │                   │  (uplink)   │
        │ ┌─────────┐ │                   │ ┌─────────┐ │
        │ │ Agent-1 │ │  ←── meta/route   │ │ Agent-3 │ │
        │ │ Agent-2 │ │      meta/spawn → │ │ Agent-4 │ │
        │ └─────────┘ │                   │ └─────────┘ │
        └─────────────┘                   └─────────────┘
```

**How it works:**

- **Agents** connect to Hub via stdio pipes (forked child processes). Each agent runs its own Kernel with LLM providers, tools, and MCP servers.
- **UI clients** (TUI, IDE via ACP, CLI) connect via TCP. Multiple clients can observe and interact with the same session simultaneously.
- **Sub-agents** are spawned on demand — Hub forks a new process, registers the parent/child relationship, and bridges completion results back to the parent as normal messages.
- **MetaHub** coordinates multiple Hubs via TCP. Cross-hub operations (routing, spawning, discovery) go through MetaHub transparently — agents don't know which Hub they're on.
- **Events** are broadcast to all UI clients. **Permissions** are raced to all connected UIs — first response wins. In cluster mode, permissions propagate through MetaHub.
- All communication uses JSON-RPC 2.0, whether over stdio or TCP.

Built as 40+ Rust crates in a layered architecture — see [CLAUDE.md](./CLAUDE.md) for the full dependency graph and development guide.

## CLI Reference

```
Usage: loopal [OPTIONS] [PROMPT]...

Arguments:
  [PROMPT]...                 Initial prompt

Options:
  -m, --model <MODEL>         Model to use
  -r, --resume <SESSION>      Resume a previous session
  -P, --permission <MODE>     Permission mode (bypass/auto/supervised)
      --plan                  Start in plan mode (read-only)
      --server                Run without TUI (server mode)
      --ephemeral             Exit after completing current task
      --worktree              Create isolated git worktree
      --no-sandbox            Disable sandbox enforcement
      --acp                   Run as ACP server
      --meta-hub <ADDR>       Run as MetaHub cluster coordinator
      --join-hub <ADDR>       Join a MetaHub cluster
      --hub-name <NAME>       Hub name when joining a cluster
  -h, --help                  Print help
```

## License

Proprietary. Copyright (c) 2024-2026 AgentsMesh.ai. All Rights Reserved.

See [LICENSE](./LICENSE) for full terms.
