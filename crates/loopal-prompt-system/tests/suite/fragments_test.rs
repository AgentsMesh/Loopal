use loopal_prompt::{FragmentRegistry, PromptBuilder, PromptContext};
use loopal_prompt_system::system_fragments;

#[test]
fn all_fragments_parse() {
    let frags = system_fragments();
    assert!(!frags.is_empty(), "should have at least 1 fragment");
    let ids: Vec<&str> = frags.iter().map(|f| f.id.as_str()).collect();
    assert!(
        ids.contains(&"core/identity"),
        "missing core/identity, got: {ids:?}"
    );
    assert!(
        ids.contains(&"core/output-efficiency"),
        "missing core/output-efficiency"
    );
    assert!(
        ids.contains(&"tools/usage-policy"),
        "missing tools/usage-policy"
    );
    assert!(
        ids.contains(&"tasks/avoid-over-engineering"),
        "missing tasks/avoid-over-engineering"
    );
}

#[test]
fn all_fragments_render() {
    let frags = system_fragments();
    let registry = FragmentRegistry::new(frags);
    let ctx = PromptContext {
        cwd: "/test/project".into(),
        platform: "linux".into(),
        date: "2026-03-22".into(),
        is_git_repo: true,
        git_branch: Some("main".into()),
        tool_names: vec![
            "Read".into(),
            "Write".into(),
            "Edit".into(),
            "Bash".into(),
            "Glob".into(),
            "Grep".into(),
        ],
        ..Default::default()
    };
    for frag in registry.fragments() {
        let rendered = registry.render(frag, &ctx);
        assert!(
            !rendered.is_empty(),
            "fragment '{}' rendered to empty string",
            frag.id
        );
    }
}

#[test]
fn full_prompt_build() {
    let frags = system_fragments();
    let registry = FragmentRegistry::new(frags);
    let builder = PromptBuilder::new(registry);
    let ctx = PromptContext {
        cwd: "/home/user/project".into(),
        platform: "darwin".into(),
        date: "2026-03-22".into(),
        is_git_repo: true,
        git_branch: Some("feature/test".into()),
        instructions: "Always respond in English.".into(),
        memory: "User prefers Rust.".into(),
        tool_names: vec!["Read".into(), "Bash".into(), "Grep".into()],
        ..Default::default()
    };
    let prompt = builder.build(&ctx);

    // Verify key sections exist
    assert!(
        prompt.contains("Always respond in English."),
        "instructions missing"
    );
    assert!(prompt.contains("# Memory"), "memory section missing");
    assert!(
        prompt.contains("User prefers Rust."),
        "memory content missing"
    );
    assert!(
        prompt.contains("Output Efficiency"),
        "output efficiency fragment missing"
    );
    assert!(
        prompt.contains("Executing Actions with Care"),
        "safety fragment missing"
    );
    // cwd is injected per-turn via env_context for root agent, not in static prompt.
    // Sub-agent fragments (which do use cwd) are excluded when is_subagent=false.
}

#[test]
fn conditional_tool_fragments() {
    let frags = system_fragments();
    let registry = FragmentRegistry::new(frags);

    // Without Bash tool — bash-guidelines and git-workflow should be excluded
    let ctx_no_bash = PromptContext::default();
    let selected = registry.select(&ctx_no_bash);
    let ids: Vec<&str> = selected.iter().map(|f| f.id.as_str()).collect();
    assert!(
        !ids.contains(&"tools/bash-guidelines"),
        "bash-guidelines should not appear without Bash tool"
    );
    assert!(
        !ids.contains(&"tools/git-workflow"),
        "git-workflow should not appear without Bash tool"
    );

    // With Bash tool — both should be included
    let ctx_bash = PromptContext {
        tool_names: vec!["Bash".into()],
        ..Default::default()
    };
    let selected = registry.select(&ctx_bash);
    let ids: Vec<&str> = selected.iter().map(|f| f.id.as_str()).collect();
    assert!(
        ids.contains(&"tools/bash-guidelines"),
        "bash-guidelines should appear with Bash tool"
    );
    assert!(
        ids.contains(&"tools/git-workflow"),
        "git-workflow should appear with Bash tool"
    );
}

#[test]
fn fragment_count() {
    let frags = system_fragments();
    // core/6 + tasks/12 + tools/6 + modes/2 + agents/3 + styles/2 = 31
    assert_eq!(
        frags.len(),
        31,
        "expected 31 fragments, got {}: {:?}",
        frags.len(),
        frags.iter().map(|f| &f.id).collect::<Vec<_>>()
    );
}
