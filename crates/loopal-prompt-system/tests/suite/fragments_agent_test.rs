use loopal_prompt::{Condition, FragmentRegistry, PromptBuilder, PromptContext};
use loopal_prompt_system::system_fragments;

#[test]
fn agent_fragments_have_correct_conditions() {
    let frags = system_fragments();
    let explore = frags.iter().find(|f| f.id == "agents/explore").unwrap();
    assert_eq!(
        explore.condition,
        Condition::Agent("explore".into()),
        "explore.md should have Agent(\"explore\") condition"
    );

    let plan = frags.iter().find(|f| f.id == "agents/plan").unwrap();
    assert_eq!(
        plan.condition,
        Condition::Agent("plan".into()),
        "plan.md should have Agent(\"plan\") condition"
    );

    let default = frags
        .iter()
        .find(|f| f.id == "agents/default-subagent")
        .unwrap();
    assert_eq!(
        default.condition,
        Condition::Always,
        "default-subagent.md should have Always condition (fallback)"
    );
}

#[test]
fn explore_subagent_gets_explore_fragment_and_core() {
    let frags = system_fragments();
    let registry = FragmentRegistry::new(frags);

    let ctx = PromptContext {
        agent_type: Some("explore".into()),
        cwd: "/project".into(),
        tool_names: vec!["Read".into(), "Grep".into(), "Glob".into(), "Bash".into()],
        ..Default::default()
    };
    let selected = registry.select(&ctx);
    let ids: Vec<&str> = selected.iter().map(|f| f.id.as_str()).collect();

    // Gets explore-specific fragment
    assert!(ids.contains(&"agents/explore"), "explore fragment missing");
    // Does NOT get default fallback
    assert!(
        !ids.contains(&"agents/default-subagent"),
        "default should be excluded when explore matches"
    );
    // Still gets core/tasks/tools fragments
    assert!(
        ids.contains(&"core/identity"),
        "core identity should be included for sub-agents"
    );
    assert!(
        ids.contains(&"tools/usage-policy"),
        "usage-policy should be included for sub-agents"
    );
}

#[test]
fn subagent_prompt_includes_core_plus_agent_fragment() {
    let frags = system_fragments();
    let registry = FragmentRegistry::new(frags);
    let builder = PromptBuilder::new(registry);

    let ctx = PromptContext {
        agent_type: Some("general".into()),
        cwd: "/work".into(),
        tool_names: vec!["Read".into(), "Bash".into()],
        ..Default::default()
    };
    let prompt = builder.build(&ctx);

    // Core behavioral fragments present
    assert!(
        prompt.contains("Output Efficiency"),
        "core fragment missing"
    );
    // Default sub-agent fragment present (fallback for "general")
    assert!(
        prompt.contains("sub-agent"),
        "default-subagent fragment missing"
    );
    // cwd rendered in default-subagent template
    assert!(prompt.contains("/work"), "cwd not rendered");
}
