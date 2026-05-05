---
name: Tool Output Efficiency
priority: 615
---
# Tool Output Efficiency

Tool calls consume context. Choose narrow commands and read outputs deliberately.

## Choose the narrow command for the narrow goal

- **Tests** — checking pass/fail only? add `--quiet`/`-q`. Investigating failures? use `--show-output`/`--no-capture` and grep `FAIL`/`panic`.
- **Diffs** — large diff? `git diff --stat` first, then `git diff -- <path>` for specific files. Never `git diff` on a huge change set blindly.
- **Logs** — only the recent end matters: `tail -n 200 <log>`, `journalctl -n 200`, `docker logs --tail 200`. Do not cat full log files.
- **Lists** — when output may be huge (`find`, `ls -R`), pipe through `| head -100` or use `wc -l` for counts first.
- **Build output** — exit code is usually enough. Only re-read full output when failures need investigation.

## Prefer specialized commands over generic fetch/cat

| Goal | Use this | Not this |
|------|---------|----------|
| GitHub PR / issue / review comments | `gh pr view`, `gh issue view`, `gh api` | `WebFetch` on the GitHub URL |
| Inspecting JSON config | `jq '.field' file.json` | Read on a 5K-line JSON |
| Lock files (`Cargo.lock`, `package-lock.json`) | `jq` / `grep` for what you need | Read on the whole file |
| Auto-generated reference (`docs.rs`, OpenAPI dumps) | `Grep` for the symbol | `WebFetch` on the index page |
| API documentation | `WebFetch` with a focused `prompt` (e.g. "find auth header format") | `WebFetch` with no prompt |

## Read code-layer strategy markers

When a tool's output is large, the code layer may apply a scenario-specific strategy and prepend metadata:

```
exit_code: 1
stdout_size: 4.2 MB
stderr_size: 612 B
applied: stack_trace_strategy
hint: 'panic detected — top frame and source frames retained'
stdout_overflow: /tmp/loopal/overflow/bash_stdout_<ts>.txt
```

- The `applied:` line tells you what the code did. If it does not match your goal (e.g. you wanted middle frames), re-read via the `*_overflow` path.
- The `hint:` line is a suggestion for next-time invocation — fold it back into your next command.
- Absence of `applied:` means the default head+tail truncation was used.

## When in doubt

Ask whether a smaller invocation gives the same answer. If yes, use it. If no, accept the larger output and rely on the strategy markers to navigate.
