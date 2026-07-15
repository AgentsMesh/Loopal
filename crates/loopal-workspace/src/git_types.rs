use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitStatus {
    pub branch: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub changes: Vec<GitChange>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitChange {
    pub path: String,
    pub index_status: String,
    pub worktree_status: String,
}

#[derive(Debug, Serialize)]
pub struct GitDiff {
    pub path: String,
    pub patch: String,
    pub original: String,
    pub modified: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Worktree {
    pub id: String,
    pub path: String,
    pub branch: Option<String>,
    pub head: String,
    pub is_main: bool,
    pub has_changes: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWorktreeParams {
    pub workspace_id: String,
    pub name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveWorktreeParams {
    pub workspace_id: String,
    pub name: String,
    pub force: bool,
}

pub(crate) struct RawWorktree {
    pub path: String,
    pub head: Option<String>,
    pub branch: Option<String>,
}
