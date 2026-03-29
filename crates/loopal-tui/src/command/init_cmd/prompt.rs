//! Prompt builder for agent-powered `/init` project analysis.

use std::path::Path;

const INIT_PROMPT: &str = r#"Analyze this project and generate a comprehensive LOOPAL.md file.

## Steps

1. **Detect project type**: Use Ls to scan the root directory. Look for:
   - Cargo.toml (Rust), package.json (Node.js), pyproject.toml / setup.py (Python),
     go.mod (Go), Makefile, CMakeLists.txt, pom.xml, build.gradle, Gemfile, etc.

2. **Read config files**: Read the detected build/config files to extract:
   - Project name and version
   - Build commands (build, test, lint, format, type-check)
   - Dependency management approach
   - Any workspace/monorepo structure

3. **Analyze project structure**: Use Ls on key directories (src/, lib/, tests/, etc.)
   - Identify module layout and layering
   - Note entry points (main.rs, index.ts, main.py, etc.)
   - Estimate code organization pattern (flat, layered, feature-based, etc.)

4. **Identify code conventions**: Read 2-3 representative source files to observe:
   - Naming style (snake_case, camelCase, PascalCase)
   - File organization patterns
   - Comment language (English, Chinese, etc.) and style
   - Config files like .editorconfig, rustfmt.toml, .eslintrc, .prettierrc, etc.

## Output

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
pub(super) fn build_init_prompt(
    cwd: &Path,
    existing_content: Option<&str>,
) -> String {
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
