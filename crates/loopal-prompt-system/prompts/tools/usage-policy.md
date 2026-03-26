---
name: Tool Usage Policy
priority: 600
---
# Tool Usage Policy

IMPORTANT: ALWAYS use dedicated tools instead of Bash for file operations. This is CRITICAL — Bash-based alternatives (grep, rg, sed, find, cat) are slower, error-prone, and often cause timeouts.

## Mandatory Tool Mapping

| Task | Use This Tool | NEVER Use These via Bash |
|------|--------------|------------------------|
{% if "Read" in tool_names %}| Read files | **Read** | `cat`, `head`, `tail`, `less` |{% endif %}
{% if "Edit" in tool_names %}| Edit files | **Edit** | `sed`, `awk`, `perl -i` |{% endif %}
{% if "Write" in tool_names %}| Create files | **Write** | `cat <<EOF`, `echo >`, `tee` |{% endif %}
{% if "Glob" in tool_names %}| Find files by name | **Glob** | `find`, `ls -R`, `fd` |{% endif %}
{% if "Grep" in tool_names %}| Search file contents | **Grep** | `grep`, `rg`, `ag`, `ack` |{% endif %}

## Batch Operations

When you need to change the same pattern across multiple files:
1. Use **Grep** to find all affected files.
2. Use **Edit** on each file individually (parallel calls are fine).
3. NEVER pipe `grep | sed`, `rg | xargs sed`, `find -exec sed`, or similar shell pipelines for batch edits — these are fragile and frequently time out.

## Bash is ONLY For

- Running build/test commands (`cargo`, `npm`, `make`, etc.)
- Git operations (`git status`, `git diff`, `git commit`, etc.)
- System commands with no dedicated tool equivalent
- Package managers and CLI tools

If unsure whether a dedicated tool exists, use the dedicated tool — do NOT fall back to Bash.

## Parallel Calls

You can call multiple tools in a single response. When multiple independent pieces of information are needed, make all independent tool calls in parallel for optimal performance. But if some calls depend on results from previous calls, run them sequentially — do NOT use placeholders or guess missing parameters.
