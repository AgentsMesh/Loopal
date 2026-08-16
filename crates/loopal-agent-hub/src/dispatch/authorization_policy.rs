pub(super) fn managed_agent_method(method: &str) -> bool {
    matches!(
        method,
        "hub/route"
            | "hub/spawn_agent"
            | "hub/wait_agent"
            | "hub/list_agents"
            | "hub/agent_info"
            | "hub/topology"
            | "hub/status"
            | "hub/mcp/list_tools"
            | "hub/mcp/call_tool"
            | "hub/mcp/snapshot"
            | "hub/audit/protected_effect"
            | "hub/audit/permission_decision"
            | "hub/secret/get"
            | "hub/secret/list_names"
            | "hub/secret/health"
    )
}

pub(super) fn workflow_worker_method(method: &str) -> bool {
    matches!(
        method,
        "hub/audit/protected_effect"
            | "hub/audit/permission_decision"
            | "hub/workflow/provider_secret/get"
            | "hub/workflow/worker_handshake"
    )
}

pub(super) fn root_agent_method(method: &str, workflow_available: bool) -> bool {
    method == "hub/mcp/reconnect" || (workflow_available && workflow_method(method))
}

fn workflow_method(method: &str) -> bool {
    matches!(
        method,
        "hub/workflow/start"
            | "hub/workflow/lookup_start"
            | "hub/workflow/get"
            | "hub/workflow/wait"
            | "hub/workflow/cancel"
    )
}

pub(super) fn external_agent_method(method: &str) -> bool {
    matches!(
        method,
        "hub/route" | "hub/list_agents" | "hub/agent_info" | "hub/topology" | "hub/status"
    )
}

pub(super) fn managed_meta_method(method: &str) -> bool {
    matches!(method, "meta/list_hubs" | "meta/topology")
}

pub(super) fn trusted_meta_method(method: &str) -> bool {
    matches!(
        method,
        "hub/spawn_remote_agent" | "hub/remote_relay" | "hub/topology"
    )
}

#[cfg(test)]
mod tests {
    use loopal_ipc::protocol::methods;

    use super::*;

    #[test]
    fn managed_acl_is_closed_set() {
        assert!(managed_agent_method(methods::HUB_SPAWN_AGENT.name));
        assert!(managed_agent_method(methods::HUB_MCP_LIST_TOOLS.name));
        assert!(!managed_agent_method(methods::HUB_MCP_RECONNECT.name));
        assert!(root_agent_method(methods::HUB_MCP_RECONNECT.name, false));
        assert!(!root_agent_method(methods::HUB_SHUTDOWN.name, true));
        assert!(!root_agent_method(methods::HUB_WORKFLOW_START.name, false));
        assert!(root_agent_method(methods::HUB_WORKFLOW_START.name, true));
        assert!(root_agent_method(
            methods::HUB_WORKFLOW_LOOKUP_START.name,
            true
        ));
        assert!(!managed_agent_method(methods::HUB_SHUTDOWN.name));
        assert!(!managed_agent_method("hub/workflow/start"));
    }

    #[test]
    fn workflow_worker_acl_is_closed_set() {
        assert!(workflow_worker_method(
            methods::HUB_AUDIT_PROTECTED_EFFECT.name
        ));
        assert!(!workflow_worker_method(methods::HUB_ROUTE.name));
        assert!(!workflow_worker_method(methods::HUB_STATUS.name));
        assert!(!workflow_worker_method(methods::META_LIST_HUBS.name));
        assert!(!workflow_worker_method(methods::HUB_SPAWN_AGENT.name));
        assert!(!workflow_worker_method(methods::HUB_MCP_LIST_TOOLS.name));
        assert!(!workflow_worker_method(methods::HUB_SECRET_GET.name));
        assert!(workflow_worker_method(
            methods::HUB_WORKFLOW_PROVIDER_SECRET_GET.name
        ));
        assert!(!workflow_worker_method(methods::HUB_WORKFLOW_START.name));
        assert!(!workflow_worker_method(
            methods::HUB_WORKFLOW_LOOKUP_START.name
        ));
        assert!(!workflow_worker_method(methods::HUB_SHUTDOWN.name));
        assert!(!workflow_worker_method("hub/future_agent_method"));
    }

    #[test]
    fn agent_meta_acl_is_closed_set() {
        assert!(managed_meta_method(methods::META_LIST_HUBS.name));
        assert!(managed_meta_method(methods::META_TOPOLOGY.name));
        assert!(!managed_meta_method(methods::META_SPAWN.name));
        assert!(!managed_meta_method("meta/future_admin"));
    }

    #[test]
    fn trusted_metahub_acl_is_closed_set() {
        assert!(trusted_meta_method(methods::HUB_SPAWN_REMOTE_AGENT.name));
        assert!(trusted_meta_method(methods::HUB_REMOTE_RELAY.name));
        assert!(trusted_meta_method(methods::HUB_TOPOLOGY.name));
        assert!(!trusted_meta_method(methods::HUB_STATUS.name));
        assert!(!trusted_meta_method("hub/future_metahub_method"));
    }
}
