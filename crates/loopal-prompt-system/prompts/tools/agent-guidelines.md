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

Your current depth in the agent tree is **{{ agent_depth }}** (0 = root, 1 = first-level sub-agent, etc.). Every sub-agent you spawn can itself spawn further sub-agents. This makes cascading delegation a real risk — if every agent subdivides its work, agent count grows exponentially and each individual agent ends up doing very little useful work.

**The deeper you are, the more you should prefer doing the work yourself:**
- At depth 0 (root): spawning sub-agents for broad tasks is natural.
- At depth 1+: you were created to handle a specific scope. Consider whether your tools (Glob, Grep, Read, Edit, Bash) are sufficient before spawning children.
- Only delegate further if your assigned scope genuinely contains multiple independent sub-problems that would each benefit from a separate context.
- Give children **narrowly scoped, concrete tasks** — not vague directives that they would need to subdivide again.

## Agent Types

- **explore**: READ-ONLY. Fast at finding files, searching code, reading content. Cannot modify anything.
- **plan**: READ-ONLY. Software architect for designing implementation plans. Cannot modify anything.
- **default** (or omit type): Full tool access. For tasks that require making changes.

## Key Rules

- Sub-agent results are NOT shown to the user — you must summarize what was found or accomplished.
- Always include a short description (3-5 words) when spawning.
- For open-ended research, use `explore` type. For implementation, use default.
- Trust agent outputs generally, but verify critical findings before acting on them.
