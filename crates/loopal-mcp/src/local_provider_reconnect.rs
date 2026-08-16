use super::LocalMcpProvider;

impl LocalMcpProvider {
    pub async fn try_reconnect(&self, server: &str) -> bool {
        self.try_reconnect_guarded(server, |commit| commit()).await
    }

    pub async fn try_reconnect_guarded<F>(&self, server: &str, guard: F) -> bool
    where
        F: FnOnce(&mut dyn FnMut()),
    {
        let plan = {
            let manager = self.manager.read().await;
            match manager.plan_reconnect(server) {
                Ok(Some(plan)) => plan,
                Ok(None) => return true,
                Err(_) => return false,
            }
        };
        let mut plan = Some(plan.connect().await);
        let committed = {
            let mut manager = self.manager.write().await;
            let mut committed = None;
            {
                let mut commit = || {
                    if let Some(plan) = plan.take() {
                        committed = Some(manager.commit_reconnect(plan));
                    }
                };
                guard(&mut commit);
            }
            committed
        };
        let Some(commit) = committed else {
            let mut retired = plan
                .expect("uncommitted reconnect must retain its candidate")
                .retire();
            retired.disconnect().await;
            return false;
        };
        if let Some(mut retired) = commit.retired {
            retired.disconnect().await;
        }
        if !commit.usable {
            tracing::warn!(server, "MCP restart_connection failed");
        }
        commit.usable
    }
}

#[cfg(test)]
#[path = "local_provider_reconnect_tests.rs"]
mod tests;
