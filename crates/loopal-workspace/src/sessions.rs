use chrono::{SecondsFormat, Utc};
use serde::Serialize;

use crate::types::WorkspaceParams;
use crate::{WorkspaceError, WorkspaceService};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopSession {
    pub id: String,
    pub title: String,
    pub model: String,
    pub mode: String,
    pub created_at: String,
    pub updated_at: String,
}

impl WorkspaceService {
    pub async fn list_sessions(
        &self,
        input: WorkspaceParams,
    ) -> Result<Vec<DesktopSession>, WorkspaceError> {
        self.require_workspace(&input.workspace_id)?;
        let store = self.session_store.clone().ok_or_else(|| {
            WorkspaceError::new("session_store_unavailable", "session store unavailable")
        })?;
        let root = self.guard.root().to_path_buf();
        let workspace_name = root
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| "Workspace".into());
        let fallback_title = format!("Loopal session · {workspace_name}");
        let sessions = tokio::task::spawn_blocking(move || store.list_root_sessions_for_cwd(&root))
            .await
            .map_err(WorkspaceError::io)?
            .map_err(WorkspaceError::io)?;
        Ok(sessions
            .into_iter()
            .map(|session| DesktopSession {
                id: session.id,
                title: if session.title.trim().is_empty() {
                    fallback_title.clone()
                } else {
                    session.title
                },
                model: session.model,
                mode: session.mode,
                created_at: session
                    .created_at
                    .with_timezone(&Utc)
                    .to_rfc3339_opts(SecondsFormat::Millis, true),
                updated_at: session
                    .updated_at
                    .with_timezone(&Utc)
                    .to_rfc3339_opts(SecondsFormat::Millis, true),
            })
            .collect())
    }
}
