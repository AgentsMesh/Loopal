//! Built-in task management tools — `TaskCreate`, `TaskUpdate`, `TaskList`,
//! `TaskGet`. Each tool lives in its own submodule; this file is just
//! the dispatch surface (re-exports + shared parsing helpers).

mod create;
mod get;
mod list;
mod update;

pub use create::TaskCreateTool;
pub use get::TaskGetTool;
pub use list::TaskListTool;
pub use update::TaskUpdateTool;

use crate::types::TaskStatus;

/// Parse the JSON `status` enum used by `TaskUpdate`.
pub(crate) fn parse_status(s: &str) -> Option<TaskStatus> {
    match s {
        "pending" => Some(TaskStatus::Pending),
        "in_progress" => Some(TaskStatus::InProgress),
        "completed" => Some(TaskStatus::Completed),
        "deleted" => Some(TaskStatus::Deleted),
        _ => None,
    }
}

/// Read a JSON `string[]` field. Missing or wrong-typed → empty `Vec`.
pub(crate) fn parse_string_array(input: &serde_json::Value, key: &str) -> Vec<String> {
    input
        .get(key)
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}
