use std::sync::Arc;

use super::states::{DispatcherReady, ListenerBound};

impl ListenerBound {
    pub async fn register_handlers(self, cli: &crate::cli::Cli) -> anyhow::Result<DispatcherReady> {
        let ListenerBound {
            hub,
            event_rx,
            listener_addr: _,
            hub_token,
        } = self;
        // reason: per-spawning-site dispatcher. This instance feeds the root
        // agent IO loop only; hub_server::{connect_local, accept_loop} each
        // build their own dispatcher (cheap: ~20 register_fn + Arc allocs).
        let dispatcher = Arc::new(loopal_agent_hub::dispatch::build_hub_dispatcher(
            hub.clone(),
        ));

        if let Some(ref meta_addr) = cli.child.join_hub {
            crate::bootstrap::uplink_bootstrap::connect_to_meta_hub(
                &hub,
                meta_addr,
                cli.child.hub_name.as_deref(),
            )
            .await?;
        }
        Ok(DispatcherReady {
            hub,
            event_rx,
            hub_token,
            dispatcher,
        })
    }
}
