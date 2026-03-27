//! Agent IPC protocol method definitions.
//!
//! Maps the existing channel-based communication to JSON-RPC methods.
//! Each method corresponds to a message type that crosses the process boundary.

/// A protocol method with its name string.
pub struct Method {
    pub name: &'static str,
}

/// All Agent IPC protocol methods.
pub mod methods {
    use super::Method;

    // ── Lifecycle ────────────────────────────────────────────────────

    /// Handshake: version negotiation and capability exchange.
    pub const INITIALIZE: Method = Method { name: "initialize" };

    /// Create a session and start the agent loop.
    pub const AGENT_START: Method = Method {
        name: "agent/start",
    };

    /// Health check / heartbeat.
    pub const AGENT_STATUS: Method = Method {
        name: "agent/status",
    };

    /// Graceful shutdown request.
    pub const AGENT_SHUTDOWN: Method = Method {
        name: "agent/shutdown",
    };

    // ── Data plane (Client → Agent) ─────────────────────────────────

    /// Send a user message or inter-agent envelope to the agent.
    /// Maps to the Envelope mailbox channel.
    pub const AGENT_MESSAGE: Method = Method {
        name: "agent/message",
    };

    // ── Control plane (Client → Agent) ──────────────────────────────

    /// Send a control command (mode switch, clear, compact, model switch, etc).
    /// Maps to the ControlCommand channel.
    pub const AGENT_CONTROL: Method = Method {
        name: "agent/control",
    };

    /// Interrupt the agent's current work (ESC or message-while-busy).
    /// Maps to InterruptSignal. Fire-and-forget notification.
    pub const AGENT_INTERRUPT: Method = Method {
        name: "agent/interrupt",
    };

    // ── Observation plane (Agent → Client) ──────────────────────────

    /// Agent event notification (stream text, tool calls, status, etc).
    /// Maps to AgentEvent channel. Fire-and-forget notification.
    pub const AGENT_EVENT: Method = Method {
        name: "agent/event",
    };

    // ── Bidirectional request/response ──────────────────────────────

    /// Agent requests tool permission from the client.
    /// Client responds with allow/deny. Maps to Permission bool channel.
    pub const AGENT_PERMISSION: Method = Method {
        name: "agent/permission",
    };

    /// Agent presents a question to the user via the client.
    /// Client responds with selected answers. Maps to UserQuestionResponse channel.
    pub const AGENT_QUESTION: Method = Method {
        name: "agent/question",
    };

    // ── Multi-client session sharing ───────────────────────────────

    /// Join an existing session as an observer.
    pub const AGENT_JOIN: Method = Method { name: "agent/join" };

    /// List active sessions on this server.
    pub const AGENT_LIST: Method = Method { name: "agent/list" };
}
