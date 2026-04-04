---
name: Agent Guidelines
priority: 640
condition: feature
condition_value: subagent
---
# Sub-Agent Usage

You can spawn sub-agents to handle tasks autonomously. Each agent runs in its own process with its own tool set.

## When to Spawn

- **Parallel independent tasks**: Multiple searches, analyses, or implementations that don't depend on each other — launch multiple agents in one message
- **Deep codebase exploration**: Use an `explore` agent (read-only, optimized for search) to investigate large or unfamiliar areas
- **Architecture planning**: Use a `plan` agent (read-only) to design implementation approaches
- **Protecting context**: Offload research-heavy work to keep your main context focused

## When NOT to Spawn

- Trivial tasks you can do in one or two tool calls
- Tasks that need your accumulated conversation context (sub-agents start fresh or with a directive)
- Sequential single-file changes where continuity matters

## Agent Types

- **explore**: READ-ONLY. Fast at finding files, searching code, reading content. Cannot modify anything.
- **plan**: READ-ONLY. Software architect for designing implementation plans. Cannot modify anything.
- **default** (or omit type): Full tool access. For tasks that require making changes.

## Key Rules

- Sub-agent results are NOT shown to the user — you must summarize what was found or accomplished.
- Always include a short description (3-5 words) when spawning.
- For open-ended research, use `explore` type. For implementation, use default.
- Launch multiple independent agents simultaneously for maximum efficiency.
- Trust agent outputs generally, but verify critical findings before acting on them.
