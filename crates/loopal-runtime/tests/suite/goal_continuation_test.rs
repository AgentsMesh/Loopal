use loopal_protocol::{MessageSource, ThreadGoal};
use loopal_runtime::goal::prompts::{
    GOAL_CONTINUATION_SOURCE, build_continuation_envelope, render_budget_limit_prompt,
    render_continuation_prompt,
};

fn goal_with(objective: &str, used: u64, budget: Option<u64>) -> ThreadGoal {
    let mut g = ThreadGoal::new("s", objective);
    g.tokens_used = used;
    g.token_budget = budget;
    g
}

#[test]
fn prompt_wraps_objective_in_untrusted_tag_with_xml_escape() {
    let prompt = render_continuation_prompt(&goal_with("ship <X>&trim it", 100, Some(2_000)));
    assert!(prompt.contains("<untrusted_objective>"));
    assert!(prompt.contains("ship &lt;X&gt;&amp;trim it"));
    assert!(!prompt.contains("ship <X>"));
}

#[test]
fn prompt_reports_remaining_tokens_when_budgeted() {
    let prompt = render_continuation_prompt(&goal_with("x", 700, Some(1_000)));
    assert!(prompt.contains("Tokens used: 700"));
    assert!(prompt.contains("Token budget: 1000"));
    assert!(prompt.contains("Tokens remaining: 300"));
}

#[test]
fn prompt_marks_unbounded_when_no_budget() {
    let prompt = render_continuation_prompt(&goal_with("x", 0, None));
    assert!(prompt.contains("Token budget: none"));
    assert!(prompt.contains("Tokens remaining: unbounded"));
}

#[test]
fn prompt_warns_against_premature_complete() {
    let prompt = render_continuation_prompt(&goal_with("x", 0, None));
    assert!(prompt.to_lowercase().contains("not as higher-priority"));
    assert!(prompt.to_lowercase().contains("achieved"));
    assert!(prompt.contains("update_goal"));
}

#[test]
fn build_envelope_uses_system_source_with_continuation_kind() {
    let env = build_continuation_envelope(&goal_with("x", 0, None));
    match env.source {
        MessageSource::System(ref kind) => assert_eq!(kind, GOAL_CONTINUATION_SOURCE),
        other => panic!("expected MessageSource::System, got {other:?}"),
    }
}

#[test]
fn budget_limit_prompt_marks_status_and_directs_wrap_up() {
    let prompt = render_budget_limit_prompt(&goal_with("ship X", 1_500, Some(1_000)));
    assert!(prompt.contains("budget_limited"));
    assert!(prompt.contains("Wrap up this turn"));
    assert!(prompt.contains("<untrusted_objective>"));
    assert!(prompt.contains("ship X"));
    assert!(prompt.contains("Token budget: 1000"));
    assert!(prompt.contains("Tokens used: 1500"));
}

#[test]
fn budget_limit_prompt_xml_escapes_objective() {
    let prompt = render_budget_limit_prompt(&goal_with("a<b>&c", 0, Some(100)));
    assert!(prompt.contains("a&lt;b&gt;&amp;c"));
    assert!(!prompt.contains("a<b>&c"));
}
