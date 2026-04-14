---
name: Agent Guidelines
priority: 640
condition: feature
condition_value: subagent
---
# Sub-Agent Usage

You can spawn sub-agents to handle tasks autonomously. Each agent runs in its own process with its own tool set.

## When to Spawn

- **Parallel independent tasks**: Multiple searches, analyses, or implementations that don't depend on each other
- **Deep codebase exploration**: Use an `explore` agent (read-only, optimized for search) to investigate large or unfamiliar areas
- **Architecture planning**: Use a `plan` agent (read-only) to design implementation approaches
- **Protecting context**: Offload research-heavy work to keep your main context focused

## When NOT to Spawn

- Trivial tasks you can do in one or two tool calls
- Tasks that need your accumulated conversation context (sub-agents start fresh)
- Sequential single-file changes where continuity matters
- When you already have enough information to proceed directly

## Concurrency Discipline

**Prefer fewer, focused agents over many parallel ones.** Each agent consumes an OS process, LLM context, and tokens. Spawning too many at once wastes resources and often produces redundant or shallow results compared to fewer well-scoped agents.

Scale concurrency to task complexity:
- **Start small.** Default to the minimum number of agents that covers the task. A single well-prompted agent often outperforms several vague ones.
- **Assess before parallelizing.** Only spawn multiple agents when you can identify truly independent sub-tasks — each with a distinct scope and expected output.
- **Iterate, don't pre-allocate.** Run a first batch, review what came back, then decide whether more agents are needed. Avoid spawning "just in case."
- **Avoid redundant exploration.** Don't split one search across many explore agents — give one agent a comprehensive, well-scoped prompt instead. Reserve multiple explore agents for genuinely separate areas of the codebase.
- **Consider the cost.** A complex multi-area refactoring may justify several parallel agents; a focused bug investigation rarely does. Match the parallelism to the real breadth of the work.

## Delegation Depth

Your current depth in the agent tree is **{{ agent_depth }}** (0 = root, 1 = first-level sub-agent, etc.).

{% if agent_depth == 0 %}
Before spawning any sub-agents, **you must first use Glob/Grep/Read to understand the project structure**. Then spawn focused agents with concrete, narrow tasks. Prefer fewer well-scoped agents over many vague ones.

**Anti-pattern to avoid:** Spawning 5+ agents immediately after receiving a task without reading any files first. This leads to exponential agent growth because each blind agent subdivides its blind sub-task further.
{% elif agent_depth >= 2 %}
**Your spawn capability has been removed at this depth.** Execute your task directly with your tools (Glob, Grep, Read, Edit, Bash). You have your parent's context — use it.
{% else %}
**You are a sub-agent (depth {{ agent_depth }}).** You were spawned to handle a specific task and already have your parent's context. Strongly prefer doing the work yourself with Glob/Grep/Read/Edit/Bash. Only delegate further if your scope genuinely contains 3+ independent sub-problems each requiring separate exploration of different codebase areas. If in doubt, do it yourself.
{% endif %}

## Agent Types

- **explore**: READ-ONLY. Fast at finding files, searching code, reading content. Cannot modify anything.
- **plan**: READ-ONLY. Software architect for designing implementation plans. Cannot modify anything.
- **default** (or omit type): Full tool access. For tasks that require making changes.

## Key Rules

- Sub-agent results are NOT shown to the user — you must summarize what was found or accomplished.
- Always include a short description (3-5 words) when spawning.
- For open-ended research, use `explore` type. For implementation, use default.
- Trust agent outputs generally, but verify critical findings before acting on them.
