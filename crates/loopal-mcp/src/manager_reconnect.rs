use loopal_error::McpError;

use crate::connection::McpConnection;
use crate::connection_generation::ConnectionGeneration;
use crate::manager::McpManager;

pub(crate) struct ReconnectPlan {
    server: String,
    observed: ConnectionGeneration,
    candidate: McpConnection,
}

pub(crate) struct ReconnectCommit {
    pub(crate) usable: bool,
    pub(crate) retired: Option<McpConnection>,
}

impl ReconnectPlan {
    pub(crate) async fn connect(mut self) -> Self {
        self.candidate.connect().await;
        self
    }

    pub(crate) fn retire(self) -> McpConnection {
        self.candidate
    }
}

impl McpManager {
    pub(crate) fn connection_generation(&self, server: &str) -> Option<ConnectionGeneration> {
        self.connections.get(server).map(McpConnection::generation)
    }

    pub(crate) fn plan_reconnect(
        &self,
        server: &str,
        failed_generation: Option<&ConnectionGeneration>,
    ) -> Result<Option<ReconnectPlan>, McpError> {
        let current = self
            .connections
            .get(server)
            .ok_or_else(|| McpError::ServerNotFound(server.to_string()))?;
        let current_request_failed =
            failed_generation.is_some_and(|generation| current.owns_generation(generation));
        if !current_request_failed && connection_is_open(current) {
            return Ok(None);
        }
        let candidate = McpConnection::new(
            current.name.clone(),
            current.config.clone(),
            self.sampling(),
        )
        .with_secret_client(self.secret_client());
        Ok(Some(ReconnectPlan {
            server: server.to_string(),
            observed: current.generation(),
            candidate,
        }))
    }

    pub(crate) fn commit_reconnect(&mut self, plan: ReconnectPlan) -> ReconnectCommit {
        let Some(current) = self.connections.get(&plan.server) else {
            return ReconnectCommit {
                usable: false,
                retired: Some(plan.candidate),
            };
        };
        if !current.owns_generation(&plan.observed) {
            return ReconnectCommit {
                usable: connection_is_open(current),
                retired: Some(plan.candidate),
            };
        }
        if !connection_is_open(&plan.candidate) {
            return ReconnectCommit {
                usable: false,
                retired: Some(plan.candidate),
            };
        }

        self.tool_map.retain(|_, owner| owner != &plan.server);
        for tool in &plan.candidate.cached_tools {
            self.tool_map.insert(tool.name.clone(), plan.server.clone());
        }
        let retired = self.connections.insert(plan.server, plan.candidate);
        ReconnectCommit {
            usable: true,
            retired,
        }
    }
}

fn connection_is_open(connection: &McpConnection) -> bool {
    connection.status.is_connected()
        && connection
            .client()
            .is_some_and(|client| !client.is_closed())
}

#[cfg(test)]
#[path = "manager_reconnect_tests.rs"]
mod tests;
