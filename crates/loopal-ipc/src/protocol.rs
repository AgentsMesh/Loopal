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

    /// Hub → Agent: request a full per-agent state dump for ViewState
    /// cold-start rebuild (Hub restart, agent reconnect, session resume).
    /// Response shape: `loopal_protocol::AgentStateSnapshot`.
    pub const AGENT_STATE_SNAPSHOT: Method = Method {
        name: "agent/state_snapshot",
    };

    // ── ViewState plane (UI ↔ Hub) ──────────────────────────────────

    /// UI → Hub: request a full ViewState snapshot for one agent.
    /// Response shape: `loopal_view_state::ViewSnapshot`.
    ///
    /// UI clients subscribe to incremental updates by listening to the
    /// existing `agent/event` notification broadcast and applying each
    /// `AgentEvent` to a local `ViewClient` reducer — there is no
    /// separate `view/delta` channel. `view/snapshot` is used only to
    /// seed the initial state on first connect or to reset after a
    /// detected event-stream gap.
    pub const VIEW_SNAPSHOT: Method = Method {
        name: "view/snapshot",
    };

    /// Hub → UI notification: the broadcast event channel lagged for
    /// this client (events were dropped). UI must re-pull view snapshots
    /// to recover state synchronization.
    pub const VIEW_RESYNC_REQUIRED: Method = Method {
        name: "view/resync_required",
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

    /// Spawn a new agent process. In-hub semantics: caller may pass `cwd`
    /// and `fork_context`; child inherits the caller's filesystem view.
    pub const HUB_SPAWN_AGENT: Method = Method {
        name: "hub/spawn_agent",
    };

    /// Spawn a new agent on this Hub on behalf of a remote (cross-hub) caller.
    /// Forwarded by MetaHub. Receiving Hub MUST use its own `default_cwd`;
    /// `cwd` / `fork_context` / `resume` fields are rejected by the handler
    /// because the caller's filesystem view is not shared.
    pub const HUB_SPAWN_REMOTE_AGENT: Method = Method {
        name: "hub/spawn_remote_agent",
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

    /// UI → Hub: respond to a `ToolPermissionRequest` event.
    ///
    /// Params: `{ agent_name: String, tool_call_id: String, allow: bool }`.
    /// Hub looks up `Hub.pending_permissions[(agent_name, tool_call_id)]`,
    /// forwards `{allow}` to the suspended agent IPC request, and emits
    /// `ToolPermissionResolved` so other UIs clear their dialogs.
    /// `agent_name` is required: same `tool_call_id` may be in flight on
    /// multiple agents simultaneously and must be disambiguated.
    pub const HUB_PERMISSION_RESPONSE: Method = Method {
        name: "hub/permission_response",
    };

    /// UI → Hub: respond to a `UserQuestionRequest` event.
    ///
    /// Params: `{ agent_name: String, question_id: String, answers: Vec<String> }`.
    /// `question_id` is generated by Hub (UUID), but `agent_name` is still
    /// part of the key for symmetry with permission and to scope cleanup.
    pub const HUB_QUESTION_RESPONSE: Method = Method {
        name: "hub/question_response",
    };

    /// Route a control command to a named agent.
    pub const HUB_CONTROL: Method = Method {
        name: "hub/control",
    };

    /// Route an interrupt signal to a named agent.
    pub const HUB_INTERRUPT: Method = Method {
        name: "hub/interrupt",
    };

    /// Query Hub status (uplink, agent count, etc).
    pub const HUB_STATUS: Method = Method { name: "hub/status" };

    // ── MetaHub methods (Sub-Hub ↔ MetaHub) ────────────────────────

    /// Sub-Hub registers with MetaHub after connecting.
    pub const META_REGISTER: Method = Method {
        name: "meta/register",
    };

    /// Sub-Hub heartbeat to MetaHub (agent count, health).
    pub const META_HEARTBEAT: Method = Method {
        name: "meta/heartbeat",
    };

    /// Cross-hub message routing (envelope forwarding).
    pub const META_ROUTE: Method = Method { name: "meta/route" };

    /// Cross-hub agent spawn delegation.
    pub const META_SPAWN: Method = Method { name: "meta/spawn" };

    /// List all connected Sub-Hubs.
    pub const META_LIST_HUBS: Method = Method {
        name: "meta/list_hubs",
    };

    /// Global agent topology across all Sub-Hubs.
    pub const META_TOPOLOGY: Method = Method {
        name: "meta/topology",
    };
}
