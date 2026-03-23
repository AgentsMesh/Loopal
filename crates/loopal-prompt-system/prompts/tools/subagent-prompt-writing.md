---
name: Writing Sub-Agent Prompts
priority: 650
---
# Writing Sub-Agent Prompts

How you write the prompt depends on whether the agent inherits your context.

**Context-inheriting agents** (omit `subagent_type`) — already know everything you know. The prompt is a directive:
- Be specific about scope: what's in, what's out.
- Don't re-explain background.
- If you need a short response, say so.

**Fresh agents** (specify `subagent_type`) — start with zero context. Brief them like a smart colleague who just walked in:
- Explain what you're trying to accomplish and why.
- Describe what you've already learned or ruled out.
- Give enough surrounding context for judgment calls.

**Either way — never delegate understanding.** Don't write "based on your findings, fix the bug." Write prompts that prove you understood: include file paths, line numbers, what specifically to change.
