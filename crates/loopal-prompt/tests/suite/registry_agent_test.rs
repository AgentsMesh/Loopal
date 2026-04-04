use loopal_prompt::{Category, Condition, Fragment, FragmentRegistry, PromptContext};

fn agent_frag(
    id: &str,
    priority: u16,
    condition: Condition,
    category: Category,
    content: &str,
) -> Fragment {
    Fragment {
        id: id.to_string(),
        name: id.to_string(),
        category,
        condition,
        priority,
        content: content.to_string(),
    }
}

#[test]
fn agent_condition_matches_only_when_type_set() {
    let frags = vec![agent_frag(
        "agents/explore",
        100,
        Condition::Agent("explore".into()),
        Category::Agents,
        "explore prompt",
    )];
    let registry = FragmentRegistry::new(frags);

    // Root agent (no agent_type) → agents excluded
    let root_ctx = PromptContext::default();
    assert!(registry.select(&root_ctx).is_empty());

    // Sub-agent with wrong type → Agent condition doesn't match
    let wrong_type = PromptContext {
        agent_type: Some("plan".into()),
        ..Default::default()
    };
    assert!(registry.select(&wrong_type).is_empty());

    // Sub-agent with correct type → matches
    let correct_type = PromptContext {
        agent_type: Some("explore".into()),
        ..Default::default()
    };
    let selected: Vec<&str> = registry
        .select(&correct_type)
        .iter()
        .map(|f| f.id.as_str())
        .collect();
    assert_eq!(selected, vec!["agents/explore"]);
}

#[test]
fn default_agent_excluded_when_specific_agent_matches() {
    let frags = vec![
        agent_frag(
            "agents/default",
            100,
            Condition::Always,
            Category::Agents,
            "default sub-agent",
        ),
        agent_frag(
            "agents/explore",
            100,
            Condition::Agent("explore".into()),
            Category::Agents,
            "explore prompt",
        ),
    ];
    let registry = FragmentRegistry::new(frags);

    // Sub-agent "general" (no specific match) → gets default only
    let general = PromptContext {
        agent_type: Some("general".into()),
        ..Default::default()
    };
    let ids: Vec<&str> = registry
        .select(&general)
        .iter()
        .map(|f| f.id.as_str())
        .collect();
    assert_eq!(ids, vec!["agents/default"]);

    // Sub-agent "explore" → gets explore, NOT default
    let explore = PromptContext {
        agent_type: Some("explore".into()),
        ..Default::default()
    };
    let ids: Vec<&str> = registry
        .select(&explore)
        .iter()
        .map(|f| f.id.as_str())
        .collect();
    assert_eq!(ids, vec!["agents/explore"]);
}

#[test]
fn agents_category_excluded_for_root() {
    let frags = vec![
        Fragment {
            id: "core/identity".into(),
            name: "core/identity".into(),
            category: Category::Core,
            condition: Condition::Always,
            priority: 100,
            content: "identity".into(),
        },
        agent_frag(
            "agents/default",
            200,
            Condition::Always,
            Category::Agents,
            "default agent",
        ),
    ];
    let registry = FragmentRegistry::new(frags);

    // Root agent → only core, no agents
    let root_ctx = PromptContext::default();
    let ids: Vec<&str> = registry
        .select(&root_ctx)
        .iter()
        .map(|f| f.id.as_str())
        .collect();
    assert_eq!(ids, vec!["core/identity"]);

    // Sub-agent → both
    let sub_ctx = PromptContext {
        agent_type: Some("general".into()),
        ..Default::default()
    };
    let ids: Vec<&str> = registry
        .select(&sub_ctx)
        .iter()
        .map(|f| f.id.as_str())
        .collect();
    assert_eq!(ids, vec!["core/identity", "agents/default"]);
}
