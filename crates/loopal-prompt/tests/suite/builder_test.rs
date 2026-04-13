use loopal_prompt::{FragmentRegistry, PromptBuilder, PromptContext};

fn frag(id: &str, priority: u16, content: &str) -> loopal_prompt::Fragment {
    loopal_prompt::Fragment {
        id: id.to_string(),
        name: id.to_string(),
        category: loopal_prompt::Category::Core,
        condition: loopal_prompt::Condition::Always,
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
    assert!(prompt.contains("Be helpful."));
    assert!(prompt.contains("# Memory"));
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
fn fragments_come_before_instructions() {
    let frags = vec![frag("core/id", 100, "## Identity")];
    let builder = PromptBuilder::new(FragmentRegistry::new(frags));
    let ctx = PromptContext {
        instructions: "## User Instructions".into(),
        ..Default::default()
    };
    let prompt = builder.build(&ctx);
    let frag_pos = prompt.find("## Identity").unwrap();
    let instr_pos = prompt.find("## User Instructions").unwrap();
    assert!(
        frag_pos < instr_pos,
        "fragments ({frag_pos}) should come before instructions ({instr_pos})"
    );
}

#[test]
fn build_memory_includes_precedence_note() {
    let builder = PromptBuilder::new(FragmentRegistry::new(vec![]));
    let ctx = PromptContext {
        memory: "Some memory content".into(),
        ..Default::default()
    };
    let prompt = builder.build(&ctx);
    assert!(
        prompt.contains("project memory takes precedence"),
        "memory section should include precedence note: {}",
        prompt
    );
}

#[test]
fn build_empty_memory_excluded() {
    let builder = PromptBuilder::new(FragmentRegistry::new(vec![]));
    let ctx = PromptContext {
        memory: String::new(),
        ..Default::default()
    };
    let prompt = builder.build(&ctx);
    assert!(
        !prompt.contains("# Memory"),
        "empty memory should not produce a section"
    );
}

#[test]
fn build_memory_at_tail() {
    let frags = vec![frag("core/id", 100, "## Identity")];
    let builder = PromptBuilder::new(FragmentRegistry::new(frags));
    let ctx = PromptContext {
        instructions: "## Instructions".into(),
        memory: "## Project Memory\nSome fact".into(),
        ..Default::default()
    };
    let prompt = builder.build(&ctx);
    let instr_pos = prompt.find("## Instructions").unwrap();
    let memory_pos = prompt.find("# Memory").unwrap();
    assert!(
        memory_pos > instr_pos,
        "memory ({memory_pos}) should come after instructions ({instr_pos})"
    );
}
