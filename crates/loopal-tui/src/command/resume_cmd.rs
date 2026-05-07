//! `/resume` — resume a previous session or open the session picker.
//!
//! With argument: hot-swap agent context to the specified session (prefix match).
//! Without argument: open a picker sub-page listing resumable sessions.

use std::path::Path;

use async_trait::async_trait;

use super::{CommandEffect, CommandHandler};
use crate::app::{App, PickerItem, PickerState, SubPage};

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
                Err(msg) => CommandEffect::Reply(msg),
            },
            None => match open_session_picker(app) {
                Ok(()) => CommandEffect::Done,
                Err(msg) => CommandEffect::Reply(msg),
            },
        }
    }
}

// ── Query ──────────────────────────────────────────────────────────

/// Resolve a partial ID (prefix) to the full session ID (root sessions only).
fn resolve_session_id(cwd: &Path, partial: &str) -> Result<String, String> {
    let sm = loopal_runtime::SessionManager::new()
        .map_err(|e| format!("Failed to access sessions: {e}"))?;
    let sessions = sm
        .list_root_sessions_for_cwd(cwd)
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

// ── Picker ─────────────────────────────────────────────────────────

fn open_session_picker(app: &mut App) -> Result<(), String> {
    let sm = loopal_runtime::SessionManager::new()
        .map_err(|_| String::from("Failed to access sessions."))?;
    let sessions = sm
        .list_root_sessions_for_cwd(&app.cwd)
        .map_err(|_| String::from("Failed to list sessions."))?;

    // Exclude current session
    let current_id = app.session.lock().root_session_id.clone();
    let items: Vec<PickerItem> = sessions
        .into_iter()
        .filter(|s| current_id.as_deref() != Some(&s.id))
        .map(|s| {
            let label = if s.title.is_empty() {
                "(untitled)".to_string()
            } else {
                s.title
            };
            let short_id = &s.id[..8.min(s.id.len())];
            let updated = s.updated_at.format("%m-%d %H:%M");
            PickerItem {
                description: format!("{short_id}  {updated}  {}", s.model),
                label,
                value: s.id,
            }
        })
        .collect();

    if items.is_empty() {
        return Err("No previous sessions found for this project.".into());
    }

    app.sub_page = Some(SubPage::SessionPicker(PickerState {
        title: "Resume Session".to_string(),
        items,
        filter: String::new(),
        filter_cursor: 0,
        selected: 0,
        thinking_options: vec![],
        thinking_selected: 0,
    }));
    Ok(())
}
