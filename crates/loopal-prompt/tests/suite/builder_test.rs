use loopal_prompt::{
    Category, Condition, Fragment, FragmentRegistry, PromptBuilder, PromptContext,
};

fn frag(id: &str, priority: u16, content: &str) -> Fragment {
    Fragment {
        id: id.to_string(),
        name: id.to_string(),
        category: Category::Core,
        condition: Condition::Always,
        priority,
        content: content.to_string(),
    }
}

#[test]
fn build_assembles_in_order() {
    let frags = vec![
        frag("second", 200, "## Second"),
        frag("first", 100, "## First"),
    ];
    let builder = PromptBuilder::new(FragmentRegistry::new(frags));
    let ctx = PromptContext::default();
    let prompt = builder.build(&ctx);
    let first_pos = prompt.find("## First").unwrap();
    let second_pos = prompt.find("## Second").unwrap();
    assert!(first_pos < second_pos);
}

#[test]
fn build_includes_instructions_and_memory() {
    let builder = PromptBuilder::new(FragmentRegistry::new(vec![]));
    let ctx = PromptContext {
        instructions: "Be helpful.".into(),
        memory: "User prefers Rust.".into(),
        ..Default::default()
    };
    let prompt = builder.build(&ctx);
    assert!(prompt.starts_with("Be helpful."));
    assert!(prompt.contains("# Project Memory"));
    assert!(prompt.contains("User prefers Rust."));
}

#[test]
fn build_skips_empty_renders() {
    let frags = vec![
        frag("cond", 100, "{% if false %}hidden{% endif %}"),
        frag("visible", 200, "visible content"),
    ];
    let builder = PromptBuilder::new(FragmentRegistry::new(frags));
    let prompt = builder.build(&PromptContext::default());
    assert!(!prompt.contains("hidden"));
    assert!(prompt.contains("visible content"));
}

#[test]
fn build_agent_prompt_fallback() {
    let builder = PromptBuilder::new(FragmentRegistry::new(vec![]));
    let ctx = PromptContext {
        agent_name: Some("explorer".into()),
        cwd: "/work".into(),
        ..Default::default()
    };
    let prompt = builder.build_agent_prompt("nonexistent", &ctx);
    assert!(prompt.contains("explorer"));
    assert!(prompt.contains("/work"));
}

#[test]
fn build_agent_prompt_uses_fragment() {
    let frags = vec![Fragment {
        id: "agents/explore".into(),
        name: "Explore".into(),
        category: Category::Agents,
        condition: Condition::Always,
        priority: 100,
        content: "You are an explorer in {{ cwd }}.".into(),
    }];
    let builder = PromptBuilder::new(FragmentRegistry::new(frags));
    let ctx = PromptContext {
        cwd: "/project".into(),
        ..Default::default()
    };
    let prompt = builder.build_agent_prompt("explore", &ctx);
    assert_eq!(prompt, "You are an explorer in /project.");
}
