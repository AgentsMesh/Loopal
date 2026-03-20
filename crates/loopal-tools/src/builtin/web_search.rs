use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult};
use serde::Deserialize;
use serde_json::{json, Value};

pub struct WebSearchTool;

#[derive(Deserialize)]
struct TavilyResponse {
    results: Vec<TavilyResult>,
}

#[derive(Deserialize)]
struct TavilyResult {
    title: String,
    url: String,
    content: String,
}

#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        "WebSearch"
    }

    fn description(&self) -> &str {
        "Search the web using Tavily API. Returns titles, URLs, and snippets for up to 10 results."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["query"],
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query"
                },
                "allowed_domains": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Only include results from these domains"
                },
                "blocked_domains": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Exclude results from these domains"
                }
            }
        })
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    async fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<ToolResult, LoopalError> {
        let query = input["query"].as_str().ok_or_else(|| {
            LoopalError::Tool(loopal_error::ToolError::InvalidInput(
                "query is required".into(),
            ))
        })?;

        let api_key = std::env::var("TAVILY_API_KEY").map_err(|_| {
            LoopalError::Tool(loopal_error::ToolError::ExecutionFailed(
                "TAVILY_API_KEY environment variable is not set".into(),
            ))
        })?;

        let include_domains = extract_string_array(&input, "allowed_domains");
        let exclude_domains = extract_string_array(&input, "blocked_domains");

        let body = json!({
            "api_key": api_key,
            "query": query,
            "include_domains": include_domains,
            "exclude_domains": exclude_domains,
            "max_results": 10,
        });

        let client = reqwest::Client::new();
        let response = client
            .post("https://api.tavily.com/search")
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                LoopalError::Tool(loopal_error::ToolError::ExecutionFailed(
                    format!("Tavily API request failed: {e}"),
                ))
            })?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_default();
            return Ok(ToolResult::error(format!(
                "Tavily API error (HTTP {status}): {text}"
            )));
        }

        let tavily: TavilyResponse = response.json().await.map_err(|e| {
            LoopalError::Tool(loopal_error::ToolError::ExecutionFailed(
                format!("Failed to parse Tavily response: {e}"),
            ))
        })?;

        if tavily.results.is_empty() {
            return Ok(ToolResult::success("No results found."));
        }

        let formatted = tavily
            .results
            .iter()
            .map(|r| format!("- [{}]({})\n  {}", r.title, r.url, r.content))
            .collect::<Vec<_>>()
            .join("\n");

        Ok(ToolResult::success(formatted))
    }
}

fn extract_string_array(input: &Value, key: &str) -> Vec<String> {
    input[key]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}
