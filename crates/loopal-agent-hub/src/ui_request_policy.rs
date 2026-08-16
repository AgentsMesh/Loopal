use loopal_ipc::protocol::methods;

pub(crate) fn is_ui_request(method: &str) -> bool {
    UI_REQUEST_METHODS.contains(&method)
}

pub(crate) fn is_control_request(method: &str) -> bool {
    CONTROL_REQUEST_METHODS.contains(&method)
}

pub(crate) fn is_recovery_request(method: &str) -> bool {
    RECOVERY_REQUEST_METHODS.contains(&method)
}

const RECOVERY_REQUEST_METHODS: &[&str] = &[
    methods::HUB_INTERRUPT.name,
    methods::HUB_SHUTDOWN.name,
    methods::HUB_SHUTDOWN_AGENT.name,
];

const CONTROL_REQUEST_METHODS: &[&str] = &[
    methods::HUB_CONTROL.name,
    methods::HUB_INTERRUPT.name,
    methods::HUB_PERMISSION_RESPONSE.name,
    methods::HUB_QUESTION_RESPONSE.name,
    methods::HUB_PLAN_APPROVAL_RESPONSE.name,
    methods::HUB_SHUTDOWN.name,
    methods::HUB_SHUTDOWN_AGENT.name,
    methods::HUB_JOIN_META.name,
    methods::HUB_LEAVE_META.name,
    methods::DESKTOP_UPDATE_SETTINGS.name,
    methods::DESKTOP_UPSERT_MCP_SERVER.name,
    methods::DESKTOP_DELETE_MCP_SERVER.name,
    methods::DESKTOP_UPSERT_SKILL.name,
    methods::DESKTOP_DELETE_SKILL.name,
];

const UI_REQUEST_METHODS: &[&str] = &[
    methods::HUB_LIST_AGENTS.name,
    methods::HUB_STATUS.name,
    methods::HUB_TOPOLOGY.name,
    methods::HUB_ROUTE.name,
    methods::HUB_CONTROL.name,
    methods::HUB_INTERRUPT.name,
    methods::HUB_PERMISSION_RESPONSE.name,
    methods::HUB_QUESTION_RESPONSE.name,
    methods::HUB_PLAN_APPROVAL_RESPONSE.name,
    methods::HUB_SHUTDOWN.name,
    methods::HUB_SHUTDOWN_AGENT.name,
    methods::HUB_JOIN_META.name,
    methods::HUB_LEAVE_META.name,
    methods::META_LIST_HUBS.name,
    methods::META_TOPOLOGY.name,
    methods::WORKSPACE_LIST_DIRECTORY.name,
    methods::WORKSPACE_READ_FILE.name,
    methods::WORKSPACE_WRITE_FILE.name,
    methods::WORKSPACE_SEARCH.name,
    methods::WORKSPACE_GIT_STATUS.name,
    methods::WORKSPACE_GIT_DIFF.name,
    methods::WORKSPACE_GIT_STAGE.name,
    methods::WORKSPACE_GIT_UNSTAGE.name,
    methods::WORKSPACE_LIST_WORKTREES.name,
    methods::WORKSPACE_CREATE_WORKTREE.name,
    methods::WORKSPACE_REMOVE_WORKTREE.name,
    methods::DESKTOP_LIST_SESSIONS.name,
    methods::DESKTOP_GET_SETTINGS.name,
    methods::DESKTOP_UPDATE_SETTINGS.name,
    methods::DESKTOP_LIST_MCP_SERVERS.name,
    methods::DESKTOP_UPSERT_MCP_SERVER.name,
    methods::DESKTOP_DELETE_MCP_SERVER.name,
    methods::DESKTOP_LIST_SKILLS.name,
    methods::DESKTOP_GET_SKILL.name,
    methods::DESKTOP_UPSERT_SKILL.name,
    methods::DESKTOP_DELETE_SKILL.name,
    methods::DESKTOP_LIST_PLUGINS.name,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_acl_denies_privileged_hub_surfaces() {
        for method in [
            "hub/secret/get",
            "hub/spawn_agent",
            "hub/mcp/list_tools",
            "agent/message",
        ] {
            assert!(!is_ui_request(method), "{method} must remain denied");
        }
    }

    #[test]
    fn ui_acl_allows_desktop_contract_only() {
        for method in UI_REQUEST_METHODS {
            assert!(is_ui_request(method));
        }
        for method in ["workspace/unknown", "desktop/unknown"] {
            assert!(!is_ui_request(method));
        }
        for method in [
            methods::HUB_CONTROL.name,
            methods::HUB_TOPOLOGY.name,
            methods::HUB_SHUTDOWN_AGENT.name,
            methods::HUB_JOIN_META.name,
            methods::META_TOPOLOGY.name,
        ] {
            assert!(is_ui_request(method));
        }
    }

    #[test]
    fn control_lane_covers_shutdown_and_interactive_responses() {
        for method in CONTROL_REQUEST_METHODS {
            assert!(is_ui_request(method));
            assert!(is_control_request(method));
        }
        assert!(!is_control_request(methods::WORKSPACE_SEARCH.name));
        assert!(is_control_request(methods::DESKTOP_UPSERT_SKILL.name));
        assert!(is_control_request(methods::DESKTOP_DELETE_SKILL.name));
        assert!(!is_control_request(methods::DESKTOP_LIST_SKILLS.name));
        assert!(!is_control_request(methods::DESKTOP_LIST_PLUGINS.name));
        for method in RECOVERY_REQUEST_METHODS {
            assert!(is_control_request(method));
            assert!(is_recovery_request(method));
        }
        assert!(!is_recovery_request(methods::HUB_CONTROL.name));
    }
}
