use std::time::Duration;

/// Caller-side declaration of "how much wall-clock time may an IPC call
/// consume, or is IPC even allowed here?".
///
/// Pass `Forbidden` from any code that runs on a latency-critical path
/// (e.g. session bootstrap before the reverse channel drains) — Remote
/// implementations of `McpProvider` / `SecretClient` will reject the
/// call immediately rather than block.
///
/// There is intentionally no `Default` impl: callers must explicitly choose
/// between `Allowed(d)` (with a justified timeout) and `Forbidden`. A blanket
/// default would silently let "I didn't think about it" callers compile,
/// defeating the purpose of making IPC cost visible at the call site.
///
/// Standard policies live in this module as named constants
/// (e.g. [`HUB_RPC_BUDGET`]). Prefer those over scattering raw `allow_secs(N)`
/// — the named constant is the single source of truth for "what is a sane
/// budget for this kind of IPC".
#[derive(Debug, Clone, Copy)]
pub enum IpcBudget {
    Allowed(Duration),
    Forbidden,
}

/// Standard budget for reverse IPC from agent → hub (mcp / secret / etc.).
///
/// 8s sits inside the layered handshake budget
/// `proxy(8s) < start_agent(20s) < HANDSHAKE(30s)` — a stuck reverse-IPC call
/// surfaces here first so the error message points at the real failure layer.
pub const HUB_RPC_BUDGET: IpcBudget = IpcBudget::Allowed(Duration::from_secs(8));

impl IpcBudget {
    pub fn allow(timeout: Duration) -> Self {
        Self::Allowed(timeout)
    }

    pub fn allow_secs(secs: u64) -> Self {
        Self::Allowed(Duration::from_secs(secs))
    }

    pub fn forbidden() -> Self {
        Self::Forbidden
    }

    /// Returns the deadline, or `None` if IPC is forbidden.
    pub fn timeout(&self) -> Option<Duration> {
        match self {
            Self::Allowed(d) => Some(*d),
            Self::Forbidden => None,
        }
    }

    pub fn is_forbidden(&self) -> bool {
        matches!(self, Self::Forbidden)
    }
}
