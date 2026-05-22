//! `/goal <new>` 在已有 active goal 上的 replace 语义测试。
//!
//! 修复前：TUI ControlCommand::GoalCreate 在已有 active goal 时被 session.create 拒绝
//! (GoalSessionError::AlreadyExists)，仅 WARN 日志、不反馈给 TUI，goal 不变。
//! 修复后：user-initiated 路径走 session.create_or_replace，直接替换 objective。
//! LLM 工具路径 (CreateGoalTool) 继续走 session.create，保持 fail-on-exists 语义。
//!
//! 测试聚焦 session 层语义。control pipeline 在 `e2e_goal_test.rs` 已有覆盖
//! (`create_command_drives_runtime_and_renders_status_bar`)；本文件直接驱动
//! `GoalRuntimeSession`，避免 HangingProvider mock 卡住 turn 时 control 队列饿死。

use std::time::Duration;

use loopal_protocol::{ThreadGoalStatus};
use loopal_tool_api::GoalSessionError;

use super::e2e_goal_support::{drain_proxy, run_goal, setup, wait_for_status};

const TIMEOUT: Duration = Duration::from_secs(2);

#[tokio::test]
async fn create_or_replace_overwrites_existing_active_goal() {
    let mut scenario = setup().await;

    // 先用 control pipeline 创建（同 create_command 测试同路径），同时验证 e2e wiring
    run_goal(&mut scenario, Some("ship feature")).await;
    wait_for_status(&mut scenario, ThreadGoalStatus::Active, TIMEOUT).await;

    let snap1 = scenario.session.snapshot().await.unwrap().unwrap();
    assert_eq!(snap1.objective, "ship feature");
    let goal_id1 = snap1.goal_id.clone();
    let updated_at_1 = snap1.updated_at;

    // 直接调 session.create_or_replace 测语义（control pipeline 在 HangingProvider 下
    // 会被 kickoff turn 卡住 —— 那是另一条独立路径的关注点）
    let snap2 = scenario
        .session
        .create_or_replace("pivot to growth".into())
        .await
        .expect("replace should succeed on active goal");

    assert_eq!(snap2.objective, "pivot to growth");
    assert_eq!(snap2.status, ThreadGoalStatus::Active);
    assert_ne!(
        snap2.goal_id, goal_id1,
        "replace 应生成新 goal_id，而非 in-place mutate"
    );
    assert!(
        snap2.updated_at >= updated_at_1,
        "updated_at 应推进；first={updated_at_1:?} second={:?}",
        snap2.updated_at
    );

    // 持久化层也应该看到新 goal
    let persisted = scenario.session.snapshot().await.unwrap().unwrap();
    assert_eq!(persisted.objective, "pivot to growth");
    assert_eq!(persisted.goal_id, snap2.goal_id);
}

#[tokio::test]
async fn create_or_replace_emits_thread_goal_updated_event() {
    let mut scenario = setup().await;

    run_goal(&mut scenario, Some("first")).await;
    wait_for_status(&mut scenario, ThreadGoalStatus::Active, TIMEOUT).await;

    drain_proxy(&mut scenario);
    let history_before = scenario.status_history.len();

    scenario
        .session
        .create_or_replace("second".into())
        .await
        .expect("replace should succeed");

    // 直接调 session 时事件会瞬间发到 proxy_rx；drain 后应看到 history 增长
    let deadline = tokio::time::Instant::now() + TIMEOUT;
    loop {
        drain_proxy(&mut scenario);
        if scenario.status_history.len() > history_before {
            return;
        }
        if tokio::time::Instant::now() >= deadline {
            panic!(
                "替换未发出 ThreadGoalUpdated 事件；history.len before={} after={}",
                history_before,
                scenario.status_history.len()
            );
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn llm_tool_create_path_still_fails_when_goal_exists() {
    // 隔离守门：确认 LLM 工具走 session.create()（不是 create_or_replace），保留
    // "Fails if a non-complete goal already exists" 的行为契约（防止退化）。
    let mut scenario = setup().await;
    run_goal(&mut scenario, Some("first")).await;
    wait_for_status(&mut scenario, ThreadGoalStatus::Active, TIMEOUT).await;

    let err = scenario
        .session
        .create("second from LLM".into())
        .await
        .expect_err("LLM-path create 在 active goal 上必须 fail");
    assert!(
        matches!(err, GoalSessionError::AlreadyExists),
        "expected AlreadyExists, got {err:?}"
    );

    // goal 内容不应被 LLM 路径误覆盖
    let snap = scenario.session.snapshot().await.unwrap().unwrap();
    assert_eq!(snap.objective, "first");
}
