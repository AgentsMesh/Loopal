//! Agent-facing methods: lifecycle, data, control, observation, view,
//! interactive, multi-client.

use super::super::Method;

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

pub const AGENT_WORKFLOW_TERMINAL: Method = Method {
    name: "agent/workflow_terminal",
};

// ── Control plane (Client → Agent) ──────────────────────────────

pub const AGENT_CONTROL: Method = Method {
    name: "agent/control",
};

/// Interrupt current work. Requests are acknowledged after the cancel signal
/// is set; legacy fire-and-forget notifications remain supported.
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

pub const AGENT_PLAN_APPROVAL: Method = Method {
    name: "agent/plan_approval",
};

/// Best-effort JSON-RPC request cancellation notification.
/// Params: `{ id: i64, method?: String }`.
pub const REQUEST_CANCEL: Method = Method {
    name: "$/cancelRequest",
};

// ── Multi-client session sharing ───────────────────────────────

pub const AGENT_JOIN: Method = Method { name: "agent/join" };
pub const AGENT_LIST: Method = Method { name: "agent/list" };

// ── MCP forwarding (Hub → root Agent) ───────────────────────────

/// Hub → root Agent: list MCP tools owned by this agent.
/// Sub-agents call `hub/mcp/list_tools` which Hub forwards here.
pub const AGENT_MCP_LIST_TOOLS: Method = Method {
    name: "agent/mcp/list_tools",
};

/// Hub → root Agent: invoke an MCP tool on this agent's behalf.
pub const AGENT_MCP_CALL_TOOL: Method = Method {
    name: "agent/mcp/call_tool",
};

/// Hub → root Agent: snapshot of this agent's MCP server statuses.
pub const AGENT_MCP_SNAPSHOT: Method = Method {
    name: "agent/mcp/snapshot",
};
