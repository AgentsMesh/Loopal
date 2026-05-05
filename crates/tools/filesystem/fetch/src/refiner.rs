use loopal_tool_api::{
    OneShotChatError, OneShotChatService, ToolContext, ToolResult, humanize_size,
};
use tracing::{info, warn};

const SYSTEM_PROMPT: &str = "You extract task-relevant facts from web pages. \
    Output is consumed by another LLM agent — be exhaustive on relevant facts, \
    ruthless on filler. Plain markdown, no preamble.";

const MAX_TOKENS: u32 = 1024;

pub async fn refine(
    chat: &dyn OneShotChatService,
    model: &str,
    user_intent: &str,
    url: &str,
    raw_markdown: &str,
) -> Result<String, OneShotChatError> {
    let user_prompt = build_user_prompt(user_intent, url, raw_markdown);
    let result = chat
        .one_shot_chat(model, SYSTEM_PROMPT, &user_prompt, MAX_TOKENS)
        .await;
    if let Err(e) = &result {
        warn!(target: "fetch_refiner", url = %url, model = %model, "refine failed: {e}");
    }
    result
}

/// Internal: orchestrates the LLM-refiner path. Public **only** so the
/// integration test in `tests/fetch_refiner_test.rs` can drive it without
/// going through `Tool::execute` (which would require mocking the network
/// backend). Not part of any stable surface.
#[doc(hidden)]
pub async fn __try_refine_internal(
    ctx: &ToolContext,
    user_intent: &str,
    url: &str,
    body: &str,
) -> Option<ToolResult> {
    let Some(chat) = ctx.one_shot_chat.as_ref() else {
        info!(target: "fetch_refiner", "skipped: no one_shot_chat in ctx");
        return None;
    };
    let Some(policy) = ctx.fetch_refiner_policy.as_ref() else {
        info!(target: "fetch_refiner", "skipped: no fetch_refiner_policy in ctx");
        return None;
    };
    let Some(model) = policy.refiner_model(body.len()) else {
        info!(
            target: "fetch_refiner",
            body_size = body.len(),
            "skipped: model_routing[refine] unset, fetch_refiner.enabled=false, or below threshold",
        );
        return None;
    };
    let summary = refine(chat.as_ref(), &model, user_intent, url, body)
        .await
        .ok()?;
    let raw_path = match crate::save_to_tmp(ctx, body, "md").await {
        Ok(p) => p,
        Err(e) => {
            warn!(target: "fetch_refiner", url = %url, "save_to_tmp failed: {e}");
            return None;
        }
    };
    let total_size = humanize_size(body.len());
    Some(ToolResult::success(format!(
        "[Refined for: {user_intent}]\nsource: {url}\nraw_size: {total_size}\nraw_path: {raw_path}\n\n--- summary ---\n{summary}"
    )))
}

fn build_user_prompt(intent: &str, url: &str, body: &str) -> String {
    format!(
        "The agent is investigating: {intent}\n\
         URL: {url}\n\n\
         Below is the page body (HTML stripped to markdown). Produce a structured digest \
         under three headings — keep only what serves the agent's intent above:\n\n\
         ## Direct Answer\n\
         The single tightest answer to the agent's intent. Quote verbatim if the page \
         states it. If the page does not address the intent, say \"Page does not directly \
         address: {intent}\".\n\n\
         ## Supporting Facts\n\
         Bullet list of corroborating facts, code snippets, version numbers, URLs. \
         Verbatim where load-bearing.\n\n\
         ## What Was Omitted\n\
         One sentence on what kind of content was skipped (navigation, unrelated \
         sections, ads, boilerplate). Do not list them.\n\n\
         Page body:\n\
         ---\n\
         {body}\n\
         ---"
    )
}
