//! Global router — cross-hub message forwarding with NAT-style hop consumption.
//!
//! Targets carry a hub path; the router consumes the front hop (DNAT)
//! before forwarding to the corresponding sub-hub. No agent-name
//! resolution is needed — the address tells the router everything.

use std::sync::Arc;

use loopal_ipc::connection::Connection;
use loopal_protocol::Envelope;

/// Cross-hub message router.
///
/// Routing decisions come from the envelope's qualified target.
/// The router does not own connections — it borrows from `HubRegistry`.
#[derive(Default)]
pub struct GlobalRouter;

impl GlobalRouter {
    pub fn new() -> Self {
        Self
    }

    /// Route an envelope: pop the next-hop hub from the target (DNAT),
    /// then deliver the envelope to that sub-hub.
    pub async fn route(
        &mut self,
        envelope: &Envelope,
        candidates: &[(&str, &Arc<Connection>)],
    ) -> Result<(), String> {
        let next_hop = envelope.target.next_hop().ok_or_else(|| {
            format!(
                "MetaHub received envelope without hub path: target='{}'. \
                 Cross-hub targets must carry an explicit `hub/agent` path.",
                envelope.target
            )
        })?;

        let conn = candidates
            .iter()
            .find(|(name, _)| *name == next_hop)
            .map(|(_, conn)| Arc::clone(conn))
            .ok_or_else(|| format!("hub '{next_hop}' not connected"))?;

        // DNAT: peel the consumed hop off target before forwarding.
        let mut forwarded = envelope.clone();
        let consumed = forwarded.apply_dnat();

        let params =
            serde_json::to_value(&forwarded).map_err(|e| format!("serialize envelope: {e}"))?;

        conn.send_request("agent/message", params)
            .await
            .map_err(|e| format!("route to hub '{next_hop}' failed: {e}"))?;

        tracing::debug!(
            hub = %next_hop,
            target = %forwarded.target,
            consumed = ?consumed,
            "routed cross-hub message (DNAT applied)"
        );
        Ok(())
    }
}
