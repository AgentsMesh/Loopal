//! `/resume` — resume a previous session or list resumable sessions.
//!
//! With argument: hot-swap agent context to the specified session (prefix match).
//! Without argument: list recent sessions for the current project directory.

use std::path::Path;

use async_trait::async_trait;

use super::{CommandEffect, CommandHandler};
use crate::app::App;

pub struct ResumeCmd;

#[async_trait]
impl CommandHandler for ResumeCmd {
    fn name(&self) -> &str {
        "/resume"
    }
    fn description(&self) -> &str {
        "Resume a previous session"
    }
    fn has_arg(&self) -> bool {
        true
    }
    async fn execute(&self, app: &mut App, arg: Option<&str>) -> CommandEffect {
        match arg {
            Some(partial_id) => match resolve_session_id(&app.cwd, partial_id) {
                Ok(full_id) => CommandEffect::ResumeSession(full_id),
                Err(msg) => {
                    app.session.push_system_message(msg);
                    CommandEffect::Done
                }
            },
            None => {
                let text = format_project_sessions(&app.cwd)
                    .unwrap_or_else(|| "No previous sessions found for this project.".into());
                app.session.push_system_message(text);
                CommandEffect::Done
            }
        }
    }
}

// ── Query ──────────────────────────────────────────────────────────

/// Resolve a partial ID (prefix) to the full session ID.
fn resolve_session_id(cwd: &Path, partial: &str) -> Result<String, String> {
    let sm = loopal_runtime::SessionManager::new()
        .map_err(|e| format!("Failed to access sessions: {e}"))?;
    let sessions = sm
        .list_sessions_for_cwd(cwd)
        .map_err(|e| format!("Failed to list sessions: {e}"))?;
    let matches: Vec<_> = sessions
        .iter()
        .filter(|s| s.id.starts_with(partial))
        .collect();
    match matches.len() {
        0 => Err(format!("No session matching '{partial}'")),
        1 => Ok(matches[0].id.clone()),
        n => Err(format!("Ambiguous: {n} sessions match '{partial}'")),
    }
}

// ── Formatting ─────────────────────────────────────────────────────

fn format_project_sessions(cwd: &Path) -> Option<String> {
    let sm = loopal_runtime::SessionManager::new().ok()?;
    let sessions = sm.list_sessions_for_cwd(cwd).ok()?;
    if sessions.is_empty() {
        return None;
    }
    let mut lines = Vec::with_capacity(sessions.len().min(5) + 3);
    lines.push("Recent sessions for this project:".into());
    lines.push(String::new());

    for s in sessions.iter().take(5) {
        let short_id = &s.id[..8];
        let updated = s.updated_at.format("%Y-%m-%d %H:%M");
        let title = if s.title.is_empty() {
            String::new()
        } else {
            format!(" — {}", s.title)
        };
        lines.push(format!("  {short_id}  {updated}  {}{title}", s.model));
    }

    lines.push(String::new());
    lines.push("To resume: /resume <ID>".into());
    Some(lines.join("\n"))
}
