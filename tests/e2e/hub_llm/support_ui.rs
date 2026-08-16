use std::sync::Arc;
use std::time::Duration;

use loopal_ipc::TcpTransport;
use loopal_ipc::connection::{Connection, Incoming, Listening};
use loopal_ipc::protocol::methods;
use loopal_ipc::transport::Transport;
use loopal_protocol::{
    AgentEvent, AgentEventPayload, Envelope, MessageSource, ROOT_AGENT_NAME, UserContent,
};
use serde_json::json;
use tokio::net::TcpStream;
use tokio::sync::mpsc::Receiver;

use super::hub::{HubHarness, TIMEOUT};

#[derive(Default, Debug)]
pub struct TurnOutcome {
    pub text: String,
    pub finished: bool,
    pub error: Option<String>,
    pub events: Vec<String>,
}

impl HubHarness {
    pub async fn second_client(&self, name: &str) -> ObserverClient {
        let (conn, rx) = register_ui_client(&self.hub_addr, &self.hub_token, name).await;
        let mut observer = ObserverClient { _conn: conn, rx };
        observer.drain_backlog().await;
        observer
    }

    pub(super) async fn drain_startup_backlog(&mut self) {
        let deadline = tokio::time::Instant::now() + TIMEOUT;
        while tokio::time::Instant::now() < deadline {
            match tokio::time::timeout(Duration::from_millis(1500), self.rx.recv()).await {
                Ok(Some(Incoming::Notification { method, params }))
                    if method == methods::AGENT_EVENT.name =>
                {
                    if let Ok(event) = serde_json::from_value::<AgentEvent>(params)
                        && matches!(event.payload, AgentEventPayload::AwaitingInput)
                    {
                        return;
                    }
                }
                Ok(Some(_)) => {}
                Ok(None) | Err(_) => return,
            }
        }
    }

    pub async fn turn(&mut self, text: &str) -> TurnOutcome {
        let envelope = Envelope::new(
            MessageSource::Human,
            ROOT_AGENT_NAME,
            UserContent::text_only(text),
        );
        self.conn
            .send_request(
                methods::HUB_ROUTE.name,
                serde_json::to_value(&envelope).unwrap(),
            )
            .await
            .expect("hub/route");

        let mut out = TurnOutcome::default();
        let deadline = tokio::time::Instant::now() + TIMEOUT;
        while tokio::time::Instant::now() < deadline {
            match tokio::time::timeout(Duration::from_secs(10), self.rx.recv()).await {
                Ok(Some(Incoming::Notification { method, params }))
                    if method == methods::AGENT_EVENT.name =>
                {
                    if let Ok(event) = serde_json::from_value::<AgentEvent>(params) {
                        // reason: child events share the stream but cannot end the root turn.
                        let root = event
                            .agent_name
                            .as_ref()
                            .map(|a| format!("{a:?}").contains(ROOT_AGENT_NAME))
                            .unwrap_or(true);
                        out.events.push(format!("{:?}", event.payload));
                        match event.payload {
                            AgentEventPayload::Stream { text } if root => out.text.push_str(&text),
                            AgentEventPayload::Error { message } if root => {
                                out.error = Some(message);
                                break;
                            }
                            AgentEventPayload::Finished | AgentEventPayload::AwaitingInput
                                if root =>
                            {
                                out.finished = true;
                                break;
                            }
                            _ => {}
                        }
                    }
                }
                Ok(Some(_)) => {}
                Ok(None) | Err(_) => break,
            }
        }
        out
    }
}

pub struct ObserverClient {
    _conn: Arc<Connection<Listening>>,
    rx: Receiver<Incoming>,
}

impl ObserverClient {
    async fn drain_backlog(&mut self) {
        let deadline = tokio::time::Instant::now() + TIMEOUT;
        while tokio::time::Instant::now() < deadline {
            match tokio::time::timeout(Duration::from_millis(1500), self.rx.recv()).await {
                Ok(Some(Incoming::Notification { method, params }))
                    if method == methods::AGENT_EVENT.name =>
                {
                    if let Ok(event) = serde_json::from_value::<AgentEvent>(params)
                        && matches!(event.payload, AgentEventPayload::AwaitingInput)
                    {
                        return;
                    }
                }
                Ok(Some(_)) => {}
                Ok(None) | Err(_) => return,
            }
        }
    }

    pub async fn collect_until_settled(&mut self, budget: Duration) -> Vec<String> {
        let mut events = Vec::new();
        let deadline = tokio::time::Instant::now() + budget;
        while tokio::time::Instant::now() < deadline {
            match tokio::time::timeout(Duration::from_secs(2), self.rx.recv()).await {
                Ok(Some(Incoming::Notification { method, params }))
                    if method == methods::AGENT_EVENT.name =>
                {
                    if let Ok(event) = serde_json::from_value::<AgentEvent>(params) {
                        events.push(format!("{:?}", event.payload));
                        if matches!(
                            event.payload,
                            AgentEventPayload::Finished | AgentEventPayload::AwaitingInput
                        ) {
                            break;
                        }
                    }
                }
                Ok(Some(_)) => {}
                Ok(None) => break,
                // reason: quiet gaps under co-load are not turn boundaries.
                Err(_) => {}
            }
        }
        events
    }
}

pub(super) async fn register_ui_client(
    addr: &str,
    token: &str,
    name: &str,
) -> (Arc<Connection<Listening>>, Receiver<Incoming>) {
    register_ui_client_with_capabilities(addr, token, name, false).await
}

pub(super) async fn register_ui_client_with_capabilities(
    addr: &str,
    token: &str,
    name: &str,
    permission: bool,
) -> (Arc<Connection<Listening>>, Receiver<Incoming>) {
    let stream = TcpStream::connect(addr).await.expect("connect hub");
    let transport: Arc<dyn Transport> = Arc::new(TcpTransport::new(stream));
    let (conn, rx) = Connection::new(transport).into_listening();
    let response = tokio::time::timeout(
        TIMEOUT,
        conn.send_request(
            methods::HUB_REGISTER.name,
            json!({
                "name": name,
                "token": token,
                "role": "ui_client",
                "capabilities": {
                    "permission": permission,
                    "question": false,
                    "plan_approval": false,
                },
            }),
        ),
    )
    .await
    .expect("hub/register timed out")
    .expect("hub/register failed");
    assert!(
        response.get("error").is_none(),
        "hub/register rejected: {response}"
    );
    (conn, rx)
}
