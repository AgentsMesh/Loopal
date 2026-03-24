use loopal_prompt::PromptContext;

// Render tests are covered via registry_test and builder_test.
// This file validates edge cases of the Minijinja rendering.

#[test]
fn render_handles_missing_variable_gracefully() {
    // Minijinja strict mode would error, but our renderer falls back to raw.
    // By default minijinja is lenient with undefined variables.
    let registry = loopal_prompt::FragmentRegistry::new(vec![loopal_prompt::Fragment {
        id: "test".into(),
        name: "test".into(),
        category: loopal_prompt::Category::Core,
        condition: loopal_prompt::Condition::Always,
        priority: 100,
        content: "Hello {{ nonexistent_var }}!".into(),
    }]);
    let ctx = PromptContext::default();
    let rendered = registry.render(&registry.fragments()[0], &ctx);
    // minijinja renders undefined as empty string by default
    assert!(rendered.contains("Hello"));
}

#[test]
fn render_jinja_condition() {
    let registry = loopal_prompt::FragmentRegistry::new(vec![loopal_prompt::Fragment {
        id: "test".into(),
        name: "test".into(),
        category: loopal_prompt::Category::Core,
        condition: loopal_prompt::Condition::Always,
        priority: 100,
        content: "{% if \"Bash\" in tool_names %}HAS_BASH{% else %}NO_BASH{% endif %}".into(),
    }]);

    let ctx_with = PromptContext {
        tool_names: vec!["Bash".into()],
        ..Default::default()
    };
    assert_eq!(
        registry.render(&registry.fragments()[0], &ctx_with),
        "HAS_BASH"
    );

    let ctx_without = PromptContext::default();
    assert_eq!(
        registry.render(&registry.fragments()[0], &ctx_without),
        "NO_BASH"
    );
}
