use loopal_prompt::{Category, Condition, Fragment, FragmentRegistry, PromptContext};

fn make_fragment(id: &str, priority: u16, condition: Condition, content: &str) -> Fragment {
    Fragment {
        id: id.to_string(),
        name: id.to_string(),
        category: Category::Core,
        condition,
        priority,
        content: content.to_string(),
    }
}

#[test]
fn select_filters_by_condition() {
    let frags = vec![
        make_fragment("always", 100, Condition::Always, "A"),
        make_fragment("plan-only", 200, Condition::Mode("plan".into()), "B"),
        make_fragment("act-only", 300, Condition::Mode("act".into()), "C"),
    ];
    let registry = FragmentRegistry::new(frags);

    let ctx = PromptContext {
        mode: "act".into(),
        ..Default::default()
    };
    let selected: Vec<&str> = registry
        .select(&ctx)
        .iter()
        .map(|f| f.id.as_str())
        .collect();
    assert_eq!(selected, vec!["always", "act-only"]);

    let ctx_plan = PromptContext {
        mode: "plan".into(),
        ..Default::default()
    };
    let selected: Vec<&str> = registry
        .select(&ctx_plan)
        .iter()
        .map(|f| f.id.as_str())
        .collect();
    assert_eq!(selected, vec!["always", "plan-only"]);
}

#[test]
fn select_sorts_by_priority() {
    let frags = vec![
        make_fragment("low", 900, Condition::Always, "L"),
        make_fragment("high", 100, Condition::Always, "H"),
        make_fragment("mid", 500, Condition::Always, "M"),
    ];
    let registry = FragmentRegistry::new(frags);
    let ctx = PromptContext::default();
    let ids: Vec<&str> = registry
        .select(&ctx)
        .iter()
        .map(|f| f.id.as_str())
        .collect();
    assert_eq!(ids, vec!["high", "mid", "low"]);
}

#[test]
fn tool_condition() {
    let frags = vec![make_fragment(
        "bash-guide",
        100,
        Condition::Tool("Bash".into()),
        "bash stuff",
    )];
    let registry = FragmentRegistry::new(frags);

    let ctx_no_bash = PromptContext::default();
    assert!(registry.select(&ctx_no_bash).is_empty());

    let ctx_bash = PromptContext {
        tool_names: vec!["Bash".into()],
        ..Default::default()
    };
    assert_eq!(registry.select(&ctx_bash).len(), 1);
}

#[test]
fn render_with_template() {
    let frags = vec![make_fragment(
        "tpl",
        100,
        Condition::Always,
        "CWD: {{ cwd }}",
    )];
    let registry = FragmentRegistry::new(frags);
    let ctx = PromptContext {
        cwd: "/home/test".into(),
        ..Default::default()
    };
    let rendered = registry.render(&registry.fragments()[0], &ctx);
    assert_eq!(rendered, "CWD: /home/test");
}
