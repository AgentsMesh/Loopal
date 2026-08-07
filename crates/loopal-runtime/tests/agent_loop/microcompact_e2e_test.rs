use std::time::Duration;

use loopal_protocol::{AgentEventPayload, CompactPhase};
use loopal_test_support::tool_history::{ToolStep, backdate_activity, tool_history_turn};
use loopal_test_support::{HarnessBuilder, chunks};
use loopal_turn::{ToolExecState, TurnStep};

const CLEARED_MARKER: &str = "[Old tool result content cleared after idle timeout]";

fn collect_tool_result_bodies(runner: &loopal_runtime::agent_loop::AgentLoopRunner) -> Vec<String> {
    let mut out = Vec::new();
    for turn in runner.turns.store().turns() {
        for step in &turn.body.steps {
            let TurnStep::ToolBatch(batch) = step else {
                continue;
            };
            for item in &batch.items {
                if let ToolExecState::Done(r) = &item.state {
                    out.push(r.content.clone());
                }
            }
        }
    }
    out
}

async fn drain_compact_phases(
    rx: &mut tokio::sync::mpsc::Receiver<loopal_protocol::AgentEvent>,
) -> Vec<CompactPhase> {
    let evts = loopal_test_support::events::drain_pending(rx).await;
    evts.into_iter()
        .filter_map(|e| match e {
            AgentEventPayload::CompactProgress { phase, .. } => Some(phase),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn microcompact_scrubs_idle_tool_results_e2e() {
    let mut h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("noop")])
        .build()
        .await;

    h.runner.params.config.microcompact_idle = Duration::from_secs(60);
    h.runner.seed_test_turns(vec![tool_history_turn(
        "go",
        vec![
            ToolStep::done("Read", "u1", "file contents A"),
            ToolStep::done("Bash", "u2", "shell output B"),
        ],
    )]);
    backdate_activity(&mut h.runner, 120);

    h.runner.check_and_microcompact().await.unwrap();

    let bodies = collect_tool_result_bodies(&h.runner);
    assert_eq!(bodies.len(), 2);
    assert!(
        bodies.iter().all(|b| b == CLEARED_MARKER),
        "all tool results scrubbed, got: {bodies:?}"
    );
    assert_eq!(
        drain_compact_phases(&mut h.event_rx).await,
        vec![CompactPhase::Microcompact, CompactPhase::Done],
        "microcompact progress must always reach a terminal Done phase"
    );
}

#[tokio::test]
async fn microcompact_noop_when_recent_activity_e2e() {
    let mut h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("noop")])
        .build()
        .await;

    h.runner.params.config.microcompact_idle = Duration::from_secs(60);
    h.runner.seed_test_turns(vec![tool_history_turn(
        "go",
        vec![ToolStep::done("Read", "u1", "stays as-is")],
    )]);

    h.runner.check_and_microcompact().await.unwrap();

    assert_eq!(collect_tool_result_bodies(&h.runner), vec!["stays as-is"]);
    assert!(
        drain_compact_phases(&mut h.event_rx).await.is_empty(),
        "no event should fire inside idle window"
    );
}

#[tokio::test]
async fn microcompact_preserves_non_scrubbable_tools_e2e() {
    let mut h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("noop")])
        .build()
        .await;

    h.runner.params.config.microcompact_idle = Duration::from_secs(60);
    h.runner.seed_test_turns(vec![tool_history_turn(
        "go",
        vec![ToolStep::done("Plan", "u1", "deep deliberation")],
    )]);
    backdate_activity(&mut h.runner, 120);

    h.runner.check_and_microcompact().await.unwrap();

    assert_eq!(
        collect_tool_result_bodies(&h.runner),
        vec!["deep deliberation"]
    );
}

#[tokio::test]
async fn microcompact_scrubs_all_supported_tool_types() {
    let tools = [
        "Read",
        "Write",
        "Edit",
        "MultiEdit",
        "Bash",
        "Grep",
        "Glob",
        "WebFetch",
        "WebSearch",
        "Ls",
    ];
    let mut h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("noop")])
        .build()
        .await;
    h.runner.params.config.microcompact_idle = Duration::from_secs(60);

    let steps: Vec<ToolStep> = tools
        .iter()
        .enumerate()
        .map(|(i, name)| ToolStep::done(name, &format!("u{i}"), &format!("{name} output body")))
        .collect();
    h.runner
        .seed_test_turns(vec![tool_history_turn("go", steps)]);
    backdate_activity(&mut h.runner, 120);

    h.runner.check_and_microcompact().await.unwrap();

    let bodies = collect_tool_result_bodies(&h.runner);
    assert_eq!(bodies.len(), tools.len());
    assert!(
        bodies.iter().all(|b| b == CLEARED_MARKER),
        "every scrubbable tool body must collapse to CLEARED_MARKER; got: {bodies:?}"
    );
}

#[tokio::test]
async fn microcompact_disabled_when_idle_duration_is_zero() {
    let mut h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("noop")])
        .build()
        .await;

    h.runner.params.config.microcompact_idle = Duration::ZERO;
    h.runner.seed_test_turns(vec![tool_history_turn(
        "go",
        vec![ToolStep::done("Read", "u1", "stays as-is")],
    )]);
    backdate_activity(&mut h.runner, 86_400);

    h.runner.check_and_microcompact().await.unwrap();

    assert_eq!(collect_tool_result_bodies(&h.runner), vec!["stays as-is"]);
    assert!(
        drain_compact_phases(&mut h.event_rx).await.is_empty(),
        "idle=0 must disable microcompact"
    );
}
