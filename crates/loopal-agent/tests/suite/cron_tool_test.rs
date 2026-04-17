//! Unit tests for CronCreate / CronDelete / CronList tools.

use loopal_agent::tools::cron::{CronCreateTool, CronDeleteTool, CronListTool};
use loopal_test_support::TestFixture;
use loopal_test_support::agent_ctx::agent_tool_context;
use loopal_tool_api::Tool;
use serde_json::json;

#[tokio::test]
async fn cron_create_valid() {
    let fixture = TestFixture::new();
    let (ctx, _shared) = agent_tool_context(&fixture);
    let result = CronCreateTool
        .execute(json!({"cron": "*/5 * * * *", "prompt": "check"}), &ctx)
        .await
        .unwrap();
    assert!(!result.is_error);
    assert!(result.content.contains("Scheduled recurring job"));
}

#[tokio::test]
async fn cron_create_oneshot() {
    let fixture = TestFixture::new();
    let (ctx, shared) = agent_tool_context(&fixture);
    let result = CronCreateTool
        .execute(
            json!({"cron": "*/5 * * * *", "prompt": "once", "recurring": false}),
            &ctx,
        )
        .await
        .unwrap();
    assert!(!result.is_error);
    assert!(result.content.contains("one-shot"));
    let jobs = shared.scheduler_handle.scheduler.list().await;
    assert!(!jobs[0].recurring);
}

#[tokio::test]
async fn cron_create_durable_flag_propagates_to_scheduler() {
    let fixture = TestFixture::new();
    let (ctx, shared) = agent_tool_context(&fixture);
    let result = CronCreateTool
        .execute(
            json!({"cron": "*/5 * * * *", "prompt": "stay", "durable": true}),
            &ctx,
        )
        .await
        .unwrap();
    assert!(!result.is_error);
    assert!(result.content.contains("(durable)"));
    let jobs = shared.scheduler_handle.scheduler.list().await;
    assert_eq!(jobs.len(), 1);
    assert!(jobs[0].durable);
}

#[tokio::test]
async fn cron_create_defaults_to_non_durable() {
    let fixture = TestFixture::new();
    let (ctx, shared) = agent_tool_context(&fixture);
    let result = CronCreateTool
        .execute(json!({"cron": "*/5 * * * *", "prompt": "transient"}), &ctx)
        .await
        .unwrap();
    assert!(!result.is_error);
    assert!(!result.content.contains("(durable)"));
    let jobs = shared.scheduler_handle.scheduler.list().await;
    assert!(!jobs[0].durable);
}

#[tokio::test]
async fn cron_create_missing_cron() {
    let fixture = TestFixture::new();
    let (ctx, _) = agent_tool_context(&fixture);
    let result = CronCreateTool
        .execute(json!({"prompt": "test"}), &ctx)
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn cron_create_empty_prompt() {
    let fixture = TestFixture::new();
    let (ctx, _) = agent_tool_context(&fixture);
    let result = CronCreateTool
        .execute(json!({"cron": "* * * * *", "prompt": ""}), &ctx)
        .await
        .unwrap();
    assert!(result.is_error);
    assert!(result.content.contains("empty"));
}

#[tokio::test]
async fn cron_create_prompt_too_long() {
    let fixture = TestFixture::new();
    let (ctx, _) = agent_tool_context(&fixture);
    let long_prompt = "x".repeat(5000);
    let result = CronCreateTool
        .execute(json!({"cron": "* * * * *", "prompt": long_prompt}), &ctx)
        .await
        .unwrap();
    assert!(result.is_error);
    assert!(result.content.contains("4096"));
}

#[tokio::test]
async fn cron_delete_existing() {
    let fixture = TestFixture::new();
    let (ctx, shared) = agent_tool_context(&fixture);
    let id = shared
        .scheduler_handle
        .scheduler
        .add("*/5 * * * *", "test", true, false)
        .await
        .unwrap();
    let result = CronDeleteTool
        .execute(json!({"id": id}), &ctx)
        .await
        .unwrap();
    assert!(!result.is_error);
    assert!(result.content.contains("Cancelled"));
}

#[tokio::test]
async fn cron_delete_nonexistent() {
    let fixture = TestFixture::new();
    let (ctx, _) = agent_tool_context(&fixture);
    let result = CronDeleteTool
        .execute(json!({"id": "nope1234"}), &ctx)
        .await
        .unwrap();
    assert!(result.is_error);
    assert!(result.content.contains("No job found"));
}

#[tokio::test]
async fn cron_delete_missing_id() {
    let fixture = TestFixture::new();
    let (ctx, _) = agent_tool_context(&fixture);
    let result = CronDeleteTool.execute(json!({}), &ctx).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn cron_list_empty() {
    let fixture = TestFixture::new();
    let (ctx, _) = agent_tool_context(&fixture);
    let result = CronListTool.execute(json!({}), &ctx).await.unwrap();
    assert!(!result.is_error);
    assert!(result.content.contains("No scheduled jobs"));
}

#[tokio::test]
async fn cron_list_with_jobs() {
    let fixture = TestFixture::new();
    let (ctx, shared) = agent_tool_context(&fixture);
    shared
        .scheduler_handle
        .scheduler
        .add("*/5 * * * *", "check", true, false)
        .await
        .unwrap();
    let result = CronListTool.execute(json!({}), &ctx).await.unwrap();
    assert!(!result.is_error);
    assert!(result.content.contains("check"));
    assert!(result.content.contains("*/5 * * * *"));
}
