use super::App;
use crate::view_client::ViewClient;

impl App {
    pub async fn seed_view_clients(&mut self) -> Result<(), String> {
        let names = self.session.fetch_agent_names().await;
        if names.is_empty() {
            return Err("hub returned no agent names".into());
        }
        self.seed_named(&names).await;
        Ok(())
    }

    pub async fn resync_view_clients(&mut self) {
        let names = self.session.fetch_agent_names().await;
        if names.is_empty() {
            tracing::warn!("resync: hub returned no agent names");
            return;
        }
        self.seed_named(&names).await;
    }

    async fn seed_named(&mut self, names: &[String]) {
        for name in names {
            match self.session.fetch_view_snapshot(name).await {
                Ok(snapshot) => match self.view_clients.get(name) {
                    Some(existing) => existing.reset_to_snapshot(snapshot),
                    None => {
                        self.view_clients.insert(
                            name.clone(),
                            ViewClient::from_snapshot(name.clone(), snapshot),
                        );
                    }
                },
                Err(e) => {
                    tracing::warn!(agent = %name, error = %e, "view/snapshot fetch failed; skipping");
                }
            }
        }
    }
}
