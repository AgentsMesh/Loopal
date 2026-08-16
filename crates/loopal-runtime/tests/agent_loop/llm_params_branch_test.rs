use std::collections::HashSet;

use loopal_runtime::AgentMode;
use loopal_runtime::agent_loop::PlanModeState;

use super::make_runner;

#[test]
fn request_tools_apply_the_global_deny_list_before_the_plan_allow_list() {
    let (mut runner, _events) = make_runner();
    runner.params.config.tool_filter = Some(HashSet::from(["Write".into()]));
    runner.params.config.mode = AgentMode::Plan;
    runner.params.config.plan_state = Some(PlanModeState {
        previous_mode: AgentMode::Act,
        previous_permission_mode: runner.params.config.permission_mode,
        tool_filter: HashSet::from(["Read".into(), "Write".into()]),
    });

    let params = runner.prepare_chat_params(None).unwrap();
    let names = params
        .tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(names, ["Read"]);
}
