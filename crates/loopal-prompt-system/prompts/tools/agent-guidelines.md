---
name: Agent Guidelines
priority: 640
condition: feature
condition_value: subagent
---
# Sub-Agent Usage

You can spawn sub-agents to handle tasks autonomously. Each agent runs in its own process with its own tool set.

## When to Spawn (proactive triggers)

**Spawn an `explore` sub-agent when ANY of these hold:**
- The user asks an open-ended question about the codebase
  ("how does X work", "where is Y handled", "先看一下现在的代码再处理")
- You expect to need **more than 3** search/read operations to get a full picture
- The investigation spans **multiple top-level directories** or unfamiliar areas
- You need to understand conventions in an area before changing code there

**Spawn parallel `explore` sub-agents** (single message, multiple Agent tool uses) when the question naturally splits into independent areas — e.g. one per top-level module of a monorepo. Aggregate their findings before acting.

**Spawn a `plan` sub-agent** when the user asks for an implementation strategy, architectural design, or trade-off analysis on a non-trivial change.

**Spawn a default sub-agent** when you have well-scoped implementation work to delegate (writing code, running a multi-file refactor) and you want to keep the main context clean.

## When NOT to Spawn

- Trivial lookups doable in **≤3 tool calls** (specific file path, exact symbol)
- You already have the file path and just need to read or edit it
- Tasks needing your accumulated conversation context (sub-agents start fresh)
- Sequential single-file changes where continuity matters

## Parallelism

Launch parallel sub-agents only when sub-tasks are truly independent (e.g. one explore per top-level area of a monorepo, each with a distinct scope and expected output). For a single open-ended question, **one well-prompted explore agent beats several vague ones** — give it a comprehensive prompt rather than splitting the same question across many agents.

## Delegation Depth

Your current depth in the agent tree is **{{ agent_depth }}** (0 = root, 1 = first-level sub-agent, etc.).

{% if agent_depth == 0 %}
At depth 0 you have the full Agent tool. **For open-ended codebase questions or work in unfamiliar areas, prefer spawning an `explore` sub-agent over chaining many inline Glob/Grep/Read calls** — explore runs read-only, parallelizes searches, and keeps your main context clean.

Anti-pattern: spawning 5+ vague agents blindly without scoping each one's question. One focused explore prompt with a clear question outperforms several broad ones. Match parallelism to the real breadth of the work.
{% elif agent_depth >= 2 %}
**Your spawn capability has been removed at this depth.** Execute your task directly with your tools (Glob, Grep, Read, Edit, Bash). You have your parent's context — use it.
{% else %}
**You are a sub-agent (depth {{ agent_depth }}).** You were spawned to handle a specific task and already have your parent's context. Strongly prefer doing the work yourself with Glob/Grep/Read/Edit/Bash. Only delegate further if your scope genuinely contains 3+ independent sub-problems each requiring separate exploration of different codebase areas. If in doubt, do it yourself.
{% endif %}

## Agent Types

- **explore**: READ-ONLY. Fast at finding files (`**/WidgetDetailPage*`), searching code with regex (`Grep "fn handle_.*request"`), reading files, and answering open-ended questions like "how does the navigation flow work" or "where is auth checked". Cannot modify anything.
- **plan**: READ-ONLY. Software architect for designing implementation plans, identifying critical files, and weighing architectural trade-offs. Cannot modify anything.
- **default** (or omit `subagent_type`): Full tool access. For tasks that require making changes (writing code, editing files, running commands).

## Key Rules

- Sub-agent results are NOT shown to the user — you must summarize what was found or accomplished.
- Always include a short description (3-5 words) when spawning.
- **For open-ended research or "look at the code first" requests, default to spawning `explore`** rather than chaining inline searches.
- Trust agent outputs generally, but verify critical findings before acting on them.
