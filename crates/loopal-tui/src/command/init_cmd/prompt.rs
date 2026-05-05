//! Prompt builder for agent-powered `/init` project analysis.

use std::path::Path;

const INIT_PROMPT: &str = r#"Analyze this project and generate a comprehensive LOOPAL.md file.

## Phase 1: Quick orientation

Use Ls on the project root. Identify:
- Project type from manifest files (Cargo.toml / package.json / pyproject.toml /
  setup.py / go.mod / Makefile / CMakeLists.txt / pom.xml / build.gradle / Gemfile)
- Top-level source / module directories
- Existing AI-tool config files (CLAUDE.md, AGENTS.md, .cursor/rules/, .cursorrules,
  .github/copilot-instructions.md) — they often contain useful but possibly stale info

## Phase 2: Deep exploration

**Choose ONE branch based on what Phase 1 found:**

### Branch A — Small single-stack project
(One manifest, ≤2 top-level source directories.)

Continue inline with Read/Grep on:
- The build/config file(s) you detected — extract project name, version, and the
  build / test / lint / format / type-check commands
- 2-3 representative source files to observe naming style, file organization,
  comment language (English / Chinese / etc.)
- Style configs if present: .editorconfig, rustfmt.toml, .eslintrc, .prettierrc

### Branch B — Medium / large project
(≥3 top-level source directories OR multiple language stacks like iOS + Go + TS.)

**You MUST spawn parallel `explore` sub-agents** — one per major area — in a
single message with multiple Agent tool uses. Each sub-agent should report:
- Build / test / lint commands specific to that area
- Naming conventions and file-organization patterns observed
- Module boundaries, key entry points, and notable internal APIs
- Area-specific gotchas (env vars, build prerequisites, codegen steps)

Wait for all sub-agents to return, then aggregate their findings before writing.

## Phase 3: Read shared root configs

Independently of Phase 2, read once at the root level: README, top-level Makefile
(if present), root package manifest, and the existing CLAUDE.md / AGENTS.md
contents (borrow but verify — these can be stale).

## Phase 4: Write LOOPAL.md

Use the **Write** tool to write the result to: `{path}/LOOPAL.md`

Follow this structure (adapt sections to what is actually relevant):

```markdown
# LOOPAL.md

This file provides guidance to Loopal when working with code in this repository.

## Build & Test Commands

```bash
# [actual build command]
# [actual test command]
# [actual lint/format command, if any]
```

## Architecture

[Concrete description based on actual directory structure and code organization]

## Code Conventions

[Actual conventions observed from the code]
```

## Important

- Only include information you can verify from the actual project files.
- Keep descriptions concise and actionable — this file is injected into the system prompt.
- If a section has no relevant content, omit it rather than writing placeholder text.
- Match the comment/documentation language used in the existing codebase.
"#;

const UPDATE_SUFFIX: &str = r#"
## Existing LOOPAL.md

The project already has a LOOPAL.md. Review it and update with more accurate or
complete information. Preserve sections that are still correct. Here is the
current content:

```
{existing}
```
"#;

/// Build the init prompt that instructs the agent to analyze the project.
///
/// When `existing_content` is `Some`, appends the current LOOPAL.md content
/// so the agent can review and update rather than overwrite blindly.
pub(super) fn build_init_prompt(cwd: &Path, existing_content: Option<&str>) -> String {
    let base = INIT_PROMPT.replace("{path}", &cwd.to_string_lossy());

    match existing_content {
        Some(content) => {
            let suffix = UPDATE_SUFFIX.replace("{existing}", content);
            format!("{base}{suffix}")
        }
        None => base,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn prompt_contains_cwd_path() {
        let cwd = PathBuf::from("/home/user/project");
        let prompt = build_init_prompt(&cwd, None);
        assert!(prompt.contains("/home/user/project/LOOPAL.md"));
    }

    #[test]
    fn prompt_includes_existing_content_when_present() {
        let cwd = PathBuf::from("/tmp/proj");
        let prompt = build_init_prompt(&cwd, Some("# Old content\nHello"));
        assert!(prompt.contains("# Old content\nHello"));
        assert!(prompt.contains("Existing LOOPAL.md"));
    }

    #[test]
    fn prompt_omits_update_suffix_when_fresh() {
        let cwd = PathBuf::from("/tmp/proj");
        let prompt = build_init_prompt(&cwd, None);
        assert!(!prompt.contains("Existing LOOPAL.md"));
    }
}
