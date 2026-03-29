//! `/init` command — agent-powered project config initialization.
//!
//! Creates scaffolding (`.loopal/` directory, `MEMORY.md`) synchronously,
//! then injects a prompt into the agent loop so the LLM can analyze the
//! project and generate a meaningful `LOOPAL.md`.

mod prompt;
mod scaffold;

use std::fs;

use async_trait::async_trait;

use super::{CommandEffect, CommandHandler};
use crate::app::App;
use prompt::build_init_prompt;
use scaffold::{display_relative, ensure_dir, write_template, MEMORY_MD_TEMPLATE};

pub struct InitCmd;

#[async_trait]
impl CommandHandler for InitCmd {
    fn name(&self) -> &str {
        "/init"
    }
    fn description(&self) -> &str {
        "Initialize project config (agent-powered)"
    }
    async fn execute(&self, app: &mut App, _arg: Option<&str>) -> CommandEffect {
        run_init(app)
    }
}

/// Run the `/init` command.
///
/// 1. Create `.loopal/` scaffolding synchronously.
/// 2. Read existing `LOOPAL.md` if present.
/// 3. Inject analysis prompt into the agent loop via `InboxPush`.
fn run_init(app: &mut App) -> CommandEffect {
    let cwd = &app.cwd;
    let mut created: Vec<String> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();

    // 1. .loopal/ directory
    let dot_dir = cwd.join(".loopal");
    ensure_dir(&dot_dir, &mut created, &mut skipped);

    // 2. .loopal/memory/MEMORY.md
    let memory_dir = dot_dir.join("memory");
    ensure_dir(&memory_dir, &mut created, &mut skipped);
    let memory_path = memory_dir.join("MEMORY.md");
    write_template(&memory_path, MEMORY_MD_TEMPLATE, &mut created, &mut skipped);

    // 3. Check existing LOOPAL.md (read but don't create)
    let instructions_path = cwd.join("LOOPAL.md");
    let existing_content = if instructions_path.exists() {
        skipped.push(display_relative(&instructions_path));
        fs::read_to_string(&instructions_path).ok()
    } else {
        None
    };

    // 4. Show scaffolding summary
    let mut lines = vec!["Initializing project...".to_string()];
    for item in &created {
        lines.push(format!("  ✓ Created {item}"));
    }
    for item in &skipped {
        lines.push(format!("  · {item} already exists"));
    }
    let action = if existing_content.is_some() {
        "Analyzing project to update LOOPAL.md..."
    } else {
        "Analyzing project to generate LOOPAL.md..."
    };
    lines.push(String::new());
    lines.push(action.to_string());
    app.session.push_system_message(lines.join("\n"));

    // 5. Build prompt and inject into agent loop
    let prompt = build_init_prompt(cwd, existing_content.as_deref());
    CommandEffect::InboxPush(prompt.into())
}
