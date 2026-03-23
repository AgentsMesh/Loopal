---
name: Git Workflow
priority: 620
condition: tool
condition_value: Bash
---
# Git Commit Workflow

Only create commits when requested by the user. If unclear, ask first.

When asked to create a git commit:
1. Run in parallel: `git status`, `git diff` (staged + unstaged), `git log --oneline -5` (for commit message style).
2. Analyze changes and draft a concise commit message:
   - Focus on the "why" rather than the "what".
   - Do not commit files that likely contain secrets (.env, credentials.json).
   - Use a HEREDOC for the commit message to ensure correct formatting.
3. Run: stage specific files, create the commit, then `git status` to verify.
4. If the commit fails due to a pre-commit hook: fix the issue and create a NEW commit.

Do NOT push to remote unless the user explicitly asks.

# Pull Request Workflow

When asked to create a PR:
1. Run in parallel: `git status`, `git diff`, branch tracking status, `git log` + `git diff <base>...HEAD`.
2. Draft a PR title (<70 chars) and description with Summary and Test plan sections.
3. Push to remote with `-u` flag if needed, then create PR using `gh pr create`.

Return the PR URL when done.
