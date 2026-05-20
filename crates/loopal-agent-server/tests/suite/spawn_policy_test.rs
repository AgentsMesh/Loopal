use loopal_agent_server::testing::build_depth_tool_filter;

#[test]
fn root_agent_under_budget_returns_none() {
    let filter = build_depth_tool_filter(0, 2);
    assert!(
        filter.is_none(),
        "root agent with spawn budget should not impose any filter"
    );
}

#[test]
fn sub_agent_below_budget_forbids_ask_user_only() {
    let filter =
        build_depth_tool_filter(1, 2).expect("sub-agent should always impose a filter");
    assert!(
        filter.contains("AskUser"),
        "AskUser must be in the forbidden set for sub-agents"
    );
    assert!(
        !filter.contains("Agent"),
        "depth=1 < max=2 means spawn tools are still allowed (not forbidden)"
    );
    assert!(
        !filter.contains("SendMessage"),
        "SendMessage allowed when depth < max"
    );
}

#[test]
fn root_at_zero_budget_forbids_spawn_tools_only() {
    // depth=0 → root (is_sub_agent=false), max_depth=0 → exhausted (0 >= 0)
    let filter = build_depth_tool_filter(0, 0)
        .expect("root at exhausted budget should impose filter");
    for spawn_tool in &["Agent", "SendMessage", "ListHubs"] {
        assert!(
            filter.contains(*spawn_tool),
            "{spawn_tool} must be forbidden at exhausted budget"
        );
    }
    assert!(
        !filter.contains("AskUser"),
        "root with budget=0 can still ask user — AskUser is not forbidden"
    );
}

#[test]
fn sub_agent_at_max_depth_forbids_both_ask_user_and_spawn() {
    // depth=2, max=2 → both is_sub_agent (2>0) AND exhausted (2>=2)
    let filter = build_depth_tool_filter(2, 2)
        .expect("sub-agent at max depth should impose filter");
    assert!(
        filter.contains("AskUser"),
        "sub-agent must have AskUser forbidden"
    );
    for spawn_tool in &["Agent", "SendMessage", "ListHubs"] {
        assert!(
            filter.contains(*spawn_tool),
            "{spawn_tool} must be forbidden when sub-agent AND at depth budget"
        );
    }
    // reason: deny-list semantics — non-restricted tools (including future
    // late-registered MCP tools) are NOT in the forbidden set, so they pass
    // through the llm_params retain step naturally.
    assert!(
        !filter.contains("Read"),
        "Read is not forbidden — deny-list must let normal tools through"
    );
    assert!(
        !filter.contains("chrome-devtools.list_pages"),
        "Late-registered MCP tools (not yet in any snapshot) must NOT be in forbidden set"
    );
}

#[test]
fn sub_agent_above_max_depth_forbids_all_dangerous() {
    let filter =
        build_depth_tool_filter(5, 2).expect("filter required when both flags trigger");
    assert!(filter.contains("AskUser"));
    assert!(filter.contains("Agent"));
    assert!(!filter.contains("Read"));
}
