use std::sync::Arc;

use async_trait::async_trait;
use loopal_error::{LoopalError, ToolError};
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult};
use serde_json::{Value, json};

use crate::event_log::EventKind;
use crate::store::MemoryGraph;

const IMPORTANCE_MIN: i64 = 1;
const IMPORTANCE_MAX: i64 = 10;

pub struct MemoryImportanceTool {
    graph: Arc<MemoryGraph>,
}

impl MemoryImportanceTool {
    pub fn new(graph: Arc<MemoryGraph>) -> Self {
        Self { graph }
    }
}

#[async_trait]
impl Tool for MemoryImportanceTool {
    fn name(&self) -> &str {
        "memory_set_importance"
    }

    fn description(&self) -> &str {
        "Set the importance (salience weight) of a memory node — boosts its \
         ranking in future memory_recall calls. Use when the user signals strong \
         preference, repeated concern, or an incident the system must remember. \
         Importance 1-10; 1 is a mild bias, 10 is dominant (overrides weak BFS \
         ranking). Tags categorize the bias; note is for human-readable rationale. \
         Effect persists across sessions via the event log."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["node", "importance"],
            "properties": {
                "node": {
                    "type": "string",
                    "description": "Memory slug to tag (must exist in .loopal/memory/<slug>.md)"
                },
                "importance": {
                    "type": "integer",
                    "minimum": IMPORTANCE_MIN,
                    "maximum": IMPORTANCE_MAX,
                    "description": "1=mild bias, 10=dominant override"
                },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Categorical tags (e.g. 'critical', 'incident', 'user-preference')"
                },
                "note": {
                    "type": "string",
                    "description": "Human-readable rationale for the tag"
                }
            }
        })
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::Write
    }

    fn secret_eligible_params(&self) -> &'static [&'static str] {
        &[]
    }

    async fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<ToolResult, LoopalError> {
        let params =
            parse_params(&input).map_err(|e| LoopalError::Tool(ToolError::InvalidInput(e)))?;

        self.graph.record_event(EventKind::ImportanceTag {
            node: params.node.clone(),
            importance: params.importance,
            tags: params.tags,
            note: params.note,
        });

        Ok(ToolResult::success(format!(
            "Tagged `{}` with importance={}",
            params.node, params.importance
        )))
    }
}

struct ImportanceParams {
    node: String,
    importance: i8,
    tags: Vec<String>,
    note: Option<String>,
}

fn parse_params(input: &Value) -> Result<ImportanceParams, String> {
    let node = input
        .get("node")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| "missing or empty `node`".to_string())?
        .to_string();

    let importance_raw = input
        .get("importance")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| "missing `importance` (integer)".to_string())?;
    if !(IMPORTANCE_MIN..=IMPORTANCE_MAX).contains(&importance_raw) {
        return Err(format!(
            "`importance` must be in {IMPORTANCE_MIN}..={IMPORTANCE_MAX}, got {importance_raw}"
        ));
    }
    let importance = importance_raw as i8;

    let tags: Vec<String> = input
        .get("tags")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|t| t.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    let note = input
        .get("note")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.to_string());

    Ok(ImportanceParams {
        node,
        importance,
        tags,
        note,
    })
}
