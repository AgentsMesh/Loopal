use loopal_ipc::protocol::methods;
use loopal_protocol::ROOT_AGENT_NAME;
use serde_json::json;

use super::hub::HubHarness;

impl HubHarness {
    pub async fn root_view_snapshot(&self) -> serde_json::Value {
        self.conn
            .send_request(
                methods::VIEW_SNAPSHOT.name,
                json!({"agent": ROOT_AGENT_NAME}),
            )
            .await
            .expect("request root view snapshot")
    }
}
