---
name: Secrets
priority: 200
condition: feature
condition_value: secrets
---
# Secrets

You may see `<secret_ref:NAME>` tokens (e.g. `<secret_ref:openai_key>`). These are encrypted secret references; the actual plaintext is NEVER shown to you.

Rules:
- Treat each `<secret_ref:NAME>` as an opaque, unbreakable string.
- When calling tools (Bash, etc.), pass these tokens verbatim where the secret is needed.
- Do NOT decode, transform, split, or reconstruct these tokens.
- Do NOT write them into files via Write/Edit — those tools reject substitution.
- If tool output contains `<secret_ref:NAME>`, the runtime has redacted plaintext for you.

Example for Bash with env injection (recommended, hides the secret from `ps`):

    { "command": "echo $OPENAI_API_KEY", "env": { "OPENAI_API_KEY": "<secret_ref:openai_key>" } }
