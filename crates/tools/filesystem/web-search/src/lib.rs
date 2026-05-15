use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, ToolContext, ToolResult, TypedTool};
use schemars::JsonSchema;
use serde::Deserialize;

pub struct WebSearchTool;

#[derive(Deserialize, JsonSchema)]
pub struct WebSearchParams {
    pub query: String,
    #[serde(default)]
    pub allowed_domains: Option<Vec<String>>,
    #[serde(default)]
    pub blocked_domains: Option<Vec<String>>,
}

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
impl TypedTool<WebSearchParams> for WebSearchTool {
    fn name(&self) -> &str {
        "WebSearch"
    }

    fn description(&self) -> &str {
        "Search the web for up-to-date information. Returns titles, URLs, and snippets.\n\
         - CRITICAL: After answering using search results, you MUST include a \"Sources:\" section with markdown hyperlinks.\n\
         - Use domain filtering (allowed_domains / blocked_domains) to scope results.\n\
         - Use the current year in queries when searching for recent information or documentation."
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    async fn execute(
        &self,
        input: WebSearchParams,
        _ctx: &ToolContext,
    ) -> Result<ToolResult, LoopalError> {
        let api_key = std::env::var("TAVILY_API_KEY").map_err(|_| {
            LoopalError::Tool(loopal_error::ToolError::ExecutionFailed(
                "TAVILY_API_KEY environment variable is not set".into(),
            ))
        })?;

        let include_domains = input.allowed_domains.unwrap_or_default();
        let exclude_domains = input.blocked_domains.unwrap_or_default();

        let body = serde_json::json!({
            "api_key": api_key,
            "query": input.query,
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
                LoopalError::Tool(loopal_error::ToolError::ExecutionFailed(format!(
                    "Tavily API request failed: {e}"
                )))
            })?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_default();
            return Ok(ToolResult::error(format!(
                "Tavily API error (HTTP {status}): {text}"
            )));
        }

        let tavily: TavilyResponse = response.json().await.map_err(|e| {
            LoopalError::Tool(loopal_error::ToolError::ExecutionFailed(format!(
                "Failed to parse Tavily response: {e}"
            )))
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
