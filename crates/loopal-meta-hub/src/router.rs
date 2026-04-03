//! Global router — cross-hub address resolution and message forwarding.
//!
//! Uses a query-cache pattern: check local cache first, then broadcast
//! `meta/resolve` to all sub-hubs on cache miss. Does NOT maintain a
//! global agent registry (avoids distributed consistency problem).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use loopal_ipc::connection::Connection;
use loopal_protocol::Envelope;

use crate::address::QualifiedAddress;

/// Cache entry with TTL.
struct CacheEntry {
    hub_name: String,
    cached_at: Instant,
}

/// Default cache TTL — entries expire after this duration.
const CACHE_TTL: Duration = Duration::from_secs(60);

/// Cross-hub address resolution and message routing.
///
/// Resolves agent addresses to hub names via:
/// 1. Local cache lookup (fast path)
/// 2. Broadcast query to all sub-hubs (slow path, on cache miss)
///
/// Does NOT own connections — borrows from `HubRegistry`.
pub struct GlobalRouter {
    /// agent_name → (hub_name, cached_at). Not authoritative.
    route_cache: HashMap<String, CacheEntry>,
}

impl Default for GlobalRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl GlobalRouter {
    pub fn new() -> Self {
        Self {
            route_cache: HashMap::new(),
        }
    }

    /// Look up cached hub name for an agent. Returns `None` on miss or expiry.
    pub fn cache_lookup(&self, agent_name: &str) -> Option<&str> {
        let entry = self.route_cache.get(agent_name)?;
        if entry.cached_at.elapsed() > CACHE_TTL {
            return None;
        }
        Some(&entry.hub_name)
    }

    /// Insert or update a cache entry.
    pub fn cache_insert(&mut self, agent_name: &str, hub_name: &str) {
        self.route_cache.insert(
            agent_name.to_string(),
            CacheEntry {
                hub_name: hub_name.to_string(),
                cached_at: Instant::now(),
            },
        );
    }

    /// Remove a cache entry (on routing failure or agent departure).
    pub fn cache_invalidate(&mut self, agent_name: &str) {
        self.route_cache.remove(agent_name);
    }

    /// Remove all cache entries for a specific hub (on hub disconnect).
    pub fn invalidate_hub(&mut self, hub_name: &str) {
        self.route_cache
            .retain(|_, entry| entry.hub_name != hub_name);
    }

    /// Evict expired entries. Call periodically to keep cache bounded.
    pub fn evict_expired(&mut self) {
        self.route_cache
            .retain(|_, entry| entry.cached_at.elapsed() <= CACHE_TTL);
    }

    /// Resolve an agent's hub by querying all sub-hubs.
    ///
    /// Sends `meta/resolve` to each hub in `candidates`. Returns the first
    /// hub that confirms the agent exists.
    pub async fn resolve_agent(
        &mut self,
        agent_name: &str,
        candidates: &[(&str, &Arc<Connection>)],
    ) -> Option<String> {
        // Fast path: cache hit
        if let Some(hub) = self.cache_lookup(agent_name) {
            return Some(hub.to_string());
        }

        // Slow path: broadcast resolve query
        let params = serde_json::json!({ "agent_name": agent_name });

        for &(hub_name, conn) in candidates {
            let result = conn.send_request("meta/resolve", params.clone()).await;
            if let Ok(resp) = result
                && resp.get("found").and_then(|v| v.as_bool()).unwrap_or(false)
            {
                self.cache_insert(agent_name, hub_name);
                return Some(hub_name.to_string());
            }
        }

        None
    }

    /// Route an envelope to the target agent's hub.
    ///
    /// Resolves the target address, then forwards the envelope to the correct
    /// sub-hub via `agent/message`.
    pub async fn route(
        &mut self,
        envelope: &Envelope,
        candidates: &[(&str, &Arc<Connection>)],
    ) -> Result<(), String> {
        let addr = QualifiedAddress::parse(&envelope.target);

        // Direct hub targeting (address contains hub prefix)
        let hub_name = if let Some(hub) = &addr.hub {
            hub.clone()
        } else {
            // Resolve agent to hub
            self.resolve_agent(&addr.agent, candidates)
                .await
                .ok_or_else(|| format!("agent '{}' not found on any sub-hub", addr.agent))?
        };

        // Find the connection for the resolved hub
        let conn = candidates
            .iter()
            .find(|(name, _)| *name == hub_name)
            .map(|(_, conn)| Arc::clone(conn))
            .ok_or_else(|| format!("hub '{hub_name}' not connected"))?;

        // Forward envelope to sub-hub (strip hub prefix from target)
        let mut forwarded = envelope.clone();
        forwarded.target = addr.agent;

        let params =
            serde_json::to_value(&forwarded).map_err(|e| format!("serialize envelope: {e}"))?;

        conn.send_request("agent/message", params)
            .await
            .map_err(|e| format!("route to hub '{hub_name}' failed: {e}"))?;

        tracing::debug!(
            hub = %hub_name,
            target = %forwarded.target,
            "routed cross-hub message"
        );
        Ok(())
    }
}
