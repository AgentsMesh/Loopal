use loopal_agent_server::testing::build_depth_tool_filter;
use loopal_config::Settings;
use loopal_kernel::Kernel;

fn kernel_with_tools() -> Kernel {
    let mut k = Kernel::new(Settings::default()).unwrap();
    loopal_agent::tools::register_all(&mut k);
    k
}

#[test]
fn root_agent_under_budget_returns_none() {
    let kernel = kernel_with_tools();
    let filter = build_depth_tool_filter(&kernel, 0, 2);
    assert!(
        filter.is_none(),
        "root agent with spawn budget should not impose any filter"
    );
}

#[test]
fn sub_agent_below_budget_removes_ask_user_only() {
    let kernel = kernel_with_tools();
    let filter = build_depth_tool_filter(&kernel, 1, 2)
        .expect("sub-agent should always impose a filter");
    assert!(
        !filter.contains("AskUser"),
        "AskUser must not be allowed in sub-agent context"
    );
    assert!(
        filter.contains("Agent"),
        "depth=1 < max=2 means spawn tools should remain available"
    );
    assert!(
        filter.contains("SendMessage"),
        "SendMessage should remain available below depth budget"
    );
}

#[test]
fn root_at_zero_budget_removes_spawn_tools_only() {
    let kernel = kernel_with_tools();
    // depth=0 → root (is_sub_agent=false), max_depth=0 → exhausted (0 >= 0)
    let filter = build_depth_tool_filter(&kernel, 0, 0)
        .expect("root at exhausted budget should impose filter");
    for spawn_tool in &["Agent", "SendMessage", "ListHubs"] {
        assert!(
            !filter.contains(*spawn_tool),
            "{spawn_tool} must be filtered at exhausted budget"
        );
    }
    assert!(
        filter.contains("AskUser"),
        "root with budget=0 can still ask user — only spawn tools stripped"
    );
}

#[test]
fn sub_agent_at_max_depth_removes_both_ask_user_and_spawn() {
    let kernel = kernel_with_tools();
    // depth=2, max=2 → both is_sub_agent (2>0) AND exhausted (2>=2)
    let filter = build_depth_tool_filter(&kernel, 2, 2)
        .expect("sub-agent at max depth should impose filter");
    assert!(
        !filter.contains("AskUser"),
        "sub-agent must not have AskUser"
    );
    for spawn_tool in &["Agent", "SendMessage", "ListHubs"] {
        assert!(
            !filter.contains(*spawn_tool),
            "{spawn_tool} must be filtered when sub-agent AND at depth budget"
        );
    }
    // Non-restricted tools survive
    assert!(filter.contains("Read"), "Read must remain available");
}

#[test]
fn sub_agent_above_max_depth_strips_all_dangerous() {
    let kernel = kernel_with_tools();
    let filter = build_depth_tool_filter(&kernel, 5, 2)
        .expect("filter required when both flags trigger");
    assert!(!filter.contains("AskUser"));
    assert!(!filter.contains("Agent"));
    assert!(filter.contains("Read"));
}
