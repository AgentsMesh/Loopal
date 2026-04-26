//! Qualified address — cross-hub agent addressing for NAT-style routing.
//!
//! Format: `"hub_1/hub_2/.../agent_name"`. Hub list grows when crossing
//! outbound hub boundaries (SNAT) and shrinks when crossing inbound (DNAT).
//! Empty hub list means a local address inside the current hub.

use std::fmt;

use serde::{Deserialize, Serialize};

/// A parsed agent address that may span across one or more hub layers.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct QualifiedAddress {
    /// Hub path from the **receiver's view**. Empty = local agent.
    /// Multi-element supports MetaHub-of-MetaHub: `["mh-1", "hub-A"]`.
    pub hub: Vec<String>,
    /// Agent name within the final hub (no `/` allowed).
    pub agent: String,
}

impl QualifiedAddress {
    /// Local (intra-hub) address.
    pub fn local(agent: impl Into<String>) -> Self {
        Self {
            hub: Vec::new(),
            agent: agent.into(),
        }
    }

    /// Cross-hub address with an explicit hub path.
    pub fn remote<H, S>(hubs: H, agent: impl Into<String>) -> Self
    where
        H: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            hub: hubs.into_iter().map(Into::into).collect(),
            agent: agent.into(),
        }
    }

    /// Parse `"h1/h2/.../agent"` — last segment is the agent, the rest is
    /// the hub path. Strings with empty segments fall back to a local
    /// address whose name preserves the original input verbatim.
    pub fn parse(s: &str) -> Self {
        if s.is_empty() {
            return Self::local("");
        }
        let parts: Vec<&str> = s.split('/').collect();
        if parts.iter().any(|p| p.is_empty()) {
            return Self::local(s);
        }
        let (agent, hubs) = parts.split_last().expect("non-empty after split");
        Self {
            hub: hubs.iter().map(|s| (*s).to_string()).collect(),
            agent: (*agent).to_string(),
        }
    }

    pub fn is_local(&self) -> bool {
        self.hub.is_empty()
    }

    pub fn is_remote(&self) -> bool {
        !self.hub.is_empty()
    }

    /// SNAT — prepend a hub name when crossing an outbound boundary.
    /// The new hub becomes the next hop from the receiver's view.
    pub fn prepend_hub(&mut self, hub_name: impl Into<String>) {
        self.hub.insert(0, hub_name.into());
    }

    /// Conditional SNAT — only prepend if this address is still local.
    /// Used by event aggregation where some inner addresses already carry
    /// a hub path (cross-hub references) and must not be double-stamped.
    pub fn prepend_hub_if_local(&mut self, hub_name: impl Into<String>) {
        if self.is_local() {
            self.prepend_hub(hub_name);
        }
    }

    /// DNAT — pop the front hub name (the next hop being consumed).
    /// Returns the popped name for routing diagnostics.
    pub fn pop_front_hub(&mut self) -> Option<String> {
        if self.hub.is_empty() {
            None
        } else {
            Some(self.hub.remove(0))
        }
    }

    /// Peek the next hop without mutating.
    pub fn next_hop(&self) -> Option<&str> {
        self.hub.first().map(String::as_str)
    }
}

impl fmt::Display for QualifiedAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for h in &self.hub {
            write!(f, "{h}/")?;
        }
        write!(f, "{}", self.agent)
    }
}

impl From<&str> for QualifiedAddress {
    fn from(s: &str) -> Self {
        Self::parse(s)
    }
}

impl From<String> for QualifiedAddress {
    fn from(s: String) -> Self {
        Self::parse(&s)
    }
}

impl From<&String> for QualifiedAddress {
    fn from(s: &String) -> Self {
        Self::parse(s)
    }
}
