use loopal_agent_hub::hub_server;

use super::states::{HubBuilt, ListenerBound};

impl HubBuilt {
    pub async fn bind_listener(self) -> anyhow::Result<ListenerBound> {
        let HubBuilt { hub, event_rx } = self;
        let (listener, port, hub_token) = hub_server::start_hub_listener(hub.clone()).await?;
        {
            let mut h = hub.lock().await;
            h.listener_port = Some(port);
            h.listener_token = Some(hub_token.clone());
        }
        let hub_accept = hub.clone();
        let token_for_loop = hub_token.clone();
        tokio::spawn(async move {
            hub_server::accept_loop(listener, hub_accept, token_for_loop).await;
        });

        Ok(ListenerBound {
            hub,
            event_rx,
            listener_addr: format!("127.0.0.1:{port}"),
            hub_token,
        })
    }
}
