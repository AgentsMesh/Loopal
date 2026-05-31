use std::sync::Arc;

use async_trait::async_trait;
use loopal_error::{LoopalError, ToolError};
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult};
use serde_json::{Value, json};

use crate::graph::{RecallParams, format_recall, recall};
use crate::store::MemoryGraph;

pub struct MemoryRecallTool {
    graph: Arc<MemoryGraph>,
}

impl MemoryRecallTool {
    pub fn new(graph: Arc<MemoryGraph>) -> Self {
        Self { graph }
    }
}

#[async_trait]
impl Tool for MemoryRecallTool {
    fn name(&self) -> &str {
        "memory_recall"
    }

    fn description(&self) -> &str {
        "THE tool for memory lookup. Always use this — never Read .loopal/memory/*.md \
         files directly. Returns directly matching memories + bidirectional N-hop neighbors \
         + co-occurring suggestions + edge trail in ONE call. \
         Set depth=0 if you only need keyword matches. \
         Provide either `query` for FTS5 search or `anchor_names` to seed traversal from \
         specific slugs (or both)."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Keyword query for FTS5 search across name/description/body"
                },
                "anchor_names": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional list of memory slugs to seed bidirectional BFS from"
                },
                "max_results": {
                    "type": "integer",
                    "default": 12,
                    "minimum": 1,
                    "maximum": 50
                },
                "depth": {
                    "type": "integer",
                    "default": 2,
                    "minimum": 0,
                    "maximum": 5,
                    "description": "BFS depth around anchors (0 = anchors only, no traversal)"
                },
                "include_synthesized": {
                    "type": "boolean",
                    "default": true,
                    "description": "Whether to include synthesized co-occurrence edges"
                },
                "min_confidence": {
                    "type": "number",
                    "default": 0.3,
                    "minimum": 0.0,
                    "maximum": 1.0
                }
            }
        })
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    fn secret_eligible_params(&self) -> &'static [&'static str] {
        &[]
    }

    async fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<ToolResult, LoopalError> {
        let params =
            parse_params(&input).map_err(|e| LoopalError::Tool(ToolError::InvalidInput(e)))?;

        if params.query.is_none() && params.anchor_names.is_empty() {
            return Ok(ToolResult::error(
                "Provide either `query` or `anchor_names`",
            ));
        }

        // reason: recall 失败（FTS syntax error / sqlite transient）应让 LLM 看到可重试的
        //  ToolResult::error，不应当成 fatal 上抛 LoopalError 中断 agent loop。
        match recall(&self.graph, &params).await {
            Ok(result) => Ok(ToolResult::success(format_recall(&result))),
            Err(e) => Ok(ToolResult::error(format!("memory_recall failed: {e}"))),
        }
    }
}

fn parse_params(input: &Value) -> Result<RecallParams, String> {
    let mut p = RecallParams::default();
    if let Some(q) = input.get("query").and_then(|v| v.as_str())
        && !q.trim().is_empty()
    {
        p.query = Some(q.to_string());
    }
    if let Some(names) = input.get("anchor_names").and_then(|v| v.as_array()) {
        for n in names {
            if let Some(s) = n.as_str() {
                p.anchor_names.push(s.to_string());
            }
        }
    }
    if let Some(n) = input.get("max_results").and_then(|v| v.as_u64()) {
        p.max_results = n.clamp(1, 50) as usize;
    }
    if let Some(d) = input.get("depth").and_then(|v| v.as_u64()) {
        p.depth = d.clamp(0, 5) as u32;
    }
    if let Some(b) = input.get("include_synthesized").and_then(|v| v.as_bool()) {
        p.include_synthesized = b;
    }
    if let Some(c) = input.get("min_confidence").and_then(|v| v.as_f64()) {
        p.min_confidence = c.clamp(0.0, 1.0) as f32;
    }
    Ok(p)
}
