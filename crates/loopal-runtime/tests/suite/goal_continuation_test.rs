use loopal_protocol::{MessageSource, ThreadGoal};
use loopal_runtime::goal::prompts::{
    GOAL_CONTINUATION_SOURCE, build_continuation_envelope, render_continuation_prompt,
};

#[test]
fn prompt_wraps_objective_in_untrusted_tag_with_xml_escape() {
    let prompt = render_continuation_prompt(&ThreadGoal::new("s", "ship <X>&trim it"));
    assert!(prompt.contains("<untrusted_objective>"));
    assert!(prompt.contains("ship &lt;X&gt;&amp;trim it"));
    assert!(!prompt.contains("ship <X>"));
}

#[test]
fn prompt_warns_against_premature_complete() {
    let prompt = render_continuation_prompt(&ThreadGoal::new("s", "x"));
    assert!(prompt.to_lowercase().contains("not as higher-priority"));
    assert!(prompt.to_lowercase().contains("achieved"));
    assert!(prompt.contains("update_goal"));
}

#[test]
fn build_envelope_uses_system_source_with_continuation_kind() {
    let env = build_continuation_envelope(&ThreadGoal::new("s", "x"));
    match env.source {
        MessageSource::System(ref kind) => assert_eq!(kind, GOAL_CONTINUATION_SOURCE),
        other => panic!("expected MessageSource::System, got {other:?}"),
    }
}
