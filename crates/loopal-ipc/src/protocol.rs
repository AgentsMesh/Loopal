//! Agent IPC protocol method definitions.
//!
//! Maps the agent communication to JSON-RPC methods.
//! Each method corresponds to a message type that crosses the process boundary.

/// A protocol method with its name string.
pub struct Method {
    pub name: &'static str,
}

/// All Agent IPC protocol methods.
pub mod methods {
    use super::Method;

    // ── Lifecycle ────────────────────────────────────────────────────

    pub const INITIALIZE: Method = Method { name: "initialize" };

    pub const AGENT_START: Method = Method {
        name: "agent/start",
    };

    pub const AGENT_STATUS: Method = Method {
        name: "agent/status",
    };

    pub const AGENT_SHUTDOWN: Method = Method {
        name: "agent/shutdown",
    };

    // ── Data plane (Client → Agent) ─────────────────────────────────

    /// Send a user message or inter-agent envelope to the agent.
    pub const AGENT_MESSAGE: Method = Method {
        name: "agent/message",
    };

    // ── Control plane (Client → Agent) ──────────────────────────────

    pub const AGENT_CONTROL: Method = Method {
        name: "agent/control",
    };

    /// Interrupt the agent's current work. Fire-and-forget notification.
    pub const AGENT_INTERRUPT: Method = Method {
        name: "agent/interrupt",
    };

    // ── Observation plane (Agent → Client) ──────────────────────────

    /// Agent event notification (stream text, tool calls, status, etc).
    pub const AGENT_EVENT: Method = Method {
        name: "agent/event",
    };

    /// Agent session completed — explicit completion signal.
    pub const AGENT_COMPLETED: Method = Method {
        name: "agent/completed",
    };

    // ── Bidirectional request/response ──────────────────────────────

    pub const AGENT_PERMISSION: Method = Method {
        name: "agent/permission",
    };

    pub const AGENT_QUESTION: Method = Method {
        name: "agent/question",
    };

    // ── Multi-client session sharing ───────────────────────────────

    pub const AGENT_JOIN: Method = Method { name: "agent/join" };
    pub const AGENT_LIST: Method = Method { name: "agent/list" };

    // ── Hub methods (Agent/Client → Hub) ─────────────────────────────

    /// Register with Hub after connecting.
    pub const HUB_REGISTER: Method = Method {
        name: "hub/register",
    };

    /// Route a point-to-point message to another agent.
    pub const HUB_ROUTE: Method = Method { name: "hub/route" };

    /// Spawn a new agent process.
    pub const HUB_SPAWN_AGENT: Method = Method {
        name: "hub/spawn_agent",
    };

    /// Wait for a spawned agent to finish and return its output.
    pub const HUB_WAIT_AGENT: Method = Method {
        name: "hub/wait_agent",
    };

    /// List all connected agents.
    pub const HUB_LIST_AGENTS: Method = Method {
        name: "hub/list_agents",
    };

    /// Query a single agent's info (lifecycle, parent, children, output).
    pub const HUB_AGENT_INFO: Method = Method {
        name: "hub/agent_info",
    };

    /// Get the full agent topology tree.
    pub const HUB_TOPOLOGY: Method = Method {
        name: "hub/topology",
    };

    /// Shut down a specific agent.
    pub const HUB_SHUTDOWN_AGENT: Method = Method {
        name: "hub/shutdown_agent",
    };

    /// Route a control command to a named agent.
    pub const HUB_CONTROL: Method = Method {
        name: "hub/control",
    };

    /// Route an interrupt signal to a named agent.
    pub const HUB_INTERRUPT: Method = Method {
        name: "hub/interrupt",
    };
}
