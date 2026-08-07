use serde::Serialize;
use serde_json::Value;

use crate::WorkspaceError;
use crate::WorkspaceService;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GitChanged<'a> {
    workspace_id: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FileChanged<'a> {
    workspace_id: &'a str,
    path: &'a str,
    kind: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ResyncRequired<'a> {
    workspace_id: &'a str,
    reason: &'a str,
}

#[derive(Debug, Clone)]
pub struct ServiceNotification {
    pub method: &'static str,
    pub params: Value,
}

impl WorkspaceService {
    pub(crate) fn publish_file_changed(&self, path: &str, kind: &str) {
        publish_file_changed(&self.events, &self.workspace_id, path, kind);
    }

    pub(crate) fn publish_git_changed(&self) {
        publish_git_changed(&self.events, &self.workspace_id);
    }
}

pub(crate) fn publish_file_changed(
    events: &tokio::sync::broadcast::Sender<ServiceNotification>,
    workspace_id: &str,
    path: &str,
    kind: &str,
) {
    let payload = FileChanged {
        workspace_id,
        path,
        kind,
    };
    if let Ok(notification) = ServiceNotification::broadcast("workspace/fileChanged", &payload) {
        let _ = events.send(notification);
    }
}

pub(crate) fn publish_git_changed(
    events: &tokio::sync::broadcast::Sender<ServiceNotification>,
    workspace_id: &str,
) {
    let payload = GitChanged { workspace_id };
    if let Ok(notification) = ServiceNotification::broadcast("workspace/gitChanged", &payload) {
        let _ = events.send(notification);
    }
}

pub(crate) fn publish_resync_required(
    events: &tokio::sync::broadcast::Sender<ServiceNotification>,
    workspace_id: &str,
    reason: &str,
) {
    let payload = ResyncRequired {
        workspace_id,
        reason,
    };
    if let Ok(notification) = ServiceNotification::broadcast("workspace/resyncRequired", &payload) {
        let _ = events.send(notification);
    }
}

impl ServiceNotification {
    pub(crate) fn broadcast<T: Serialize>(
        method: &'static str,
        params: &T,
    ) -> Result<Self, WorkspaceError> {
        Ok(Self {
            method,
            params: serde_json::to_value(params).map_err(WorkspaceError::io)?,
        })
    }
}
