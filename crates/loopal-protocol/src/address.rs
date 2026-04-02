//! Qualified address — cross-hub agent addressing.
//!
//! Addresses follow the format `"hub_name/agent_name"` for cross-hub routing,
//! or plain `"agent_name"` for local (intra-hub) routing.

use std::fmt;

/// A parsed agent address that may span across hubs.
///
/// - `hub: None` — local address within the current hub.
/// - `hub: Some(name)` — cross-hub address targeting a specific sub-hub.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct QualifiedAddress {
    /// Target hub name. `None` means local (current hub).
    pub hub: Option<String>,
    /// Agent name within the target hub.
    pub agent: String,
}

impl QualifiedAddress {
    /// Create a local (intra-hub) address.
    pub fn local(agent: impl Into<String>) -> Self {
        Self {
            hub: None,
            agent: agent.into(),
        }
    }

    /// Create a cross-hub address.
    pub fn remote(hub: impl Into<String>, agent: impl Into<String>) -> Self {
        Self {
            hub: Some(hub.into()),
            agent: agent.into(),
        }
    }

    /// Parse an address string.
    ///
    /// - `"researcher"` → local address
    /// - `"hub-a/researcher"` → cross-hub address
    ///
    /// Hub names must not contain `/`.
    pub fn parse(s: &str) -> Self {
        match s.split_once('/') {
            Some((hub, agent)) if !hub.is_empty() && !agent.is_empty() => Self {
                hub: Some(hub.to_string()),
                agent: agent.to_string(),
            },
            _ => Self {
                hub: None,
                agent: s.to_string(),
            },
        }
    }

    /// Whether this is a cross-hub address.
    pub fn is_remote(&self) -> bool {
        self.hub.is_some()
    }
}

impl fmt::Display for QualifiedAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.hub {
            Some(hub) => write!(f, "{hub}/{}", self.agent),
            None => write!(f, "{}", self.agent),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_local_address() {
        let addr = QualifiedAddress::parse("researcher");
        assert_eq!(addr.hub, None);
        assert_eq!(addr.agent, "researcher");
        assert!(!addr.is_remote());
        assert_eq!(addr.to_string(), "researcher");
    }

    #[test]
    fn parse_remote_address() {
        let addr = QualifiedAddress::parse("hub-a/researcher");
        assert_eq!(addr.hub.as_deref(), Some("hub-a"));
        assert_eq!(addr.agent, "researcher");
        assert!(addr.is_remote());
        assert_eq!(addr.to_string(), "hub-a/researcher");
    }

    #[test]
    fn parse_edge_cases() {
        // Leading slash → local (empty hub name)
        let addr = QualifiedAddress::parse("/researcher");
        assert_eq!(addr.hub, None);
        assert_eq!(addr.agent, "/researcher");

        // Trailing slash → local (empty agent name)
        let addr = QualifiedAddress::parse("hub-a/");
        assert_eq!(addr.hub, None);
        assert_eq!(addr.agent, "hub-a/");

        // Empty string → local
        let addr = QualifiedAddress::parse("");
        assert_eq!(addr.hub, None);
        assert_eq!(addr.agent, "");
    }

    #[test]
    fn constructors() {
        let local = QualifiedAddress::local("main");
        assert!(!local.is_remote());

        let remote = QualifiedAddress::remote("hub-b", "worker");
        assert!(remote.is_remote());
        assert_eq!(remote.to_string(), "hub-b/worker");
    }
}
