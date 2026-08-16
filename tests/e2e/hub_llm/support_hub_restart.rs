use loopal_ipc::protocol::methods;
use loopal_protocol::WorkflowRunId;
use serde_json::Value;

use super::{HubEnv, HubHarness, TIMEOUT};

impl HubHarness {
    pub async fn crash_with_resume(mut self, scenario: Value) -> Self {
        self._child
            .start_kill()
            .expect("force-kill Hub before resume");
        let status = tokio::time::timeout(TIMEOUT, self._child.wait())
            .await
            .expect("force-killed Hub did not exit before resume")
            .expect("wait for force-killed Hub before resume");
        assert!(!status.success(), "force-killed Hub exited successfully");

        let HubHarness {
            session_id,
            mock,
            _home: home,
            _cwd: cwd,
            agent_binary_override,
            permission_mode,
            ..
        } = self;
        drop(mock);
        Self::launch_resume(
            HubEnv {
                home,
                cwd,
                agent_binary_override,
                permission_mode,
            },
            scenario,
            false,
            Some(session_id),
        )
        .await
    }

    pub async fn restart_with_resume(mut self, scenario: Value) -> Self {
        self.conn
            .send_request(methods::HUB_SHUTDOWN.name, serde_json::json!({}))
            .await
            .expect("request Hub shutdown before resume");
        tokio::time::timeout(TIMEOUT, self._child.wait())
            .await
            .expect("Hub shutdown timed out before resume")
            .expect("wait for Hub shutdown before resume");

        let HubHarness {
            session_id,
            mock,
            _home: home,
            _cwd: cwd,
            agent_binary_override,
            permission_mode,
            ..
        } = self;
        drop(mock);
        Self::launch_resume(
            HubEnv {
                home,
                cwd,
                agent_binary_override,
                permission_mode,
            },
            scenario,
            false,
            Some(session_id),
        )
        .await
    }

    pub async fn restart_with_missing_delivery_ack(
        mut self,
        run_id: &WorkflowRunId,
        scenario: Value,
    ) -> Self {
        self.conn
            .send_request(methods::HUB_SHUTDOWN.name, serde_json::json!({}))
            .await
            .expect("request Hub shutdown before ACK-loss resume");
        tokio::time::timeout(TIMEOUT, self._child.wait())
            .await
            .expect("Hub shutdown timed out before ACK-loss resume")
            .expect("wait for Hub shutdown before ACK-loss resume");
        self.remove_delivery_ack(run_id);

        let HubHarness {
            session_id,
            mock,
            _home: home,
            _cwd: cwd,
            agent_binary_override,
            permission_mode,
            ..
        } = self;
        drop(mock);
        Self::launch_resume(
            HubEnv {
                home,
                cwd,
                agent_binary_override,
                permission_mode,
            },
            scenario,
            false,
            Some(session_id),
        )
        .await
    }
}
