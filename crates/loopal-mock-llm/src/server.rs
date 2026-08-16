use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{Result, ensure};
use serde::Deserialize;
use serde_json::Value;
use serde_json::json;
use tokio::net::{TcpListener, TcpStream};

use crate::Scenario;
use crate::http::{read_request, write_json};
use crate::messages::handle_completion;
use crate::protocol::route;
use crate::state::ServerState;

pub struct ServerConfig {
    pub bind: SocketAddr,
    pub scenario: Scenario,
    pub api_key: String,
}

pub async fn run(config: ServerConfig) -> Result<()> {
    ensure!(
        config.bind.ip().is_loopback(),
        "mock LLM must bind to loopback"
    );
    let listener = TcpListener::bind(config.bind).await?;
    let base_url = format!("http://{}", listener.local_addr()?);
    println!(
        "LOOPAL_MOCK_LLM {}",
        json!({
            "baseUrl": base_url,
            "protocols": ["anthropic", "openai_responses", "openai_compat", "google"],
            "version": 3
        })
    );
    serve(listener, config.scenario, config.api_key).await
}

pub async fn serve(listener: TcpListener, scenario: Scenario, api_key: String) -> Result<()> {
    ensure!(
        listener.local_addr()?.ip().is_loopback(),
        "mock LLM must bind to loopback"
    );
    let state = Arc::new(ServerState::new(scenario, api_key));
    loop {
        let (stream, _) = listener.accept().await?;
        let state = state.clone();
        tokio::spawn(async move {
            if let Err(error) = handle_connection(stream, state).await {
                eprintln!("mock LLM connection error: {error}");
            }
        });
    }
}

async fn handle_connection(mut stream: TcpStream, state: Arc<ServerState>) -> Result<()> {
    let request = match read_request(&mut stream).await {
        Ok(value) => value,
        Err(error) => {
            return write_json(
                &mut stream,
                400,
                &json!({"error": error.to_string()}),
                &BTreeMap::new(),
            )
            .await;
        }
    };
    let path = request.path.split('?').next().unwrap_or(&request.path);
    match (request.method.as_str(), path) {
        ("GET", "/health" | "/__mock/health") => {
            write_json(
                &mut stream,
                200,
                &json!({"status": "ready"}),
                &BTreeMap::new(),
            )
            .await
        }
        ("GET", "/v1/models") => write_models(&mut stream).await,
        ("GET", "/__mock/requests") => {
            let requests = state.requests().await;
            write_json(
                &mut stream,
                200,
                &serde_json::to_value(requests)?,
                &BTreeMap::new(),
            )
            .await
        }
        ("GET", "/__mock/state" | "/__mock/verify") => {
            let snapshot = state.snapshot().await;
            write_json(
                &mut stream,
                200,
                &serde_json::to_value(snapshot)?,
                &BTreeMap::new(),
            )
            .await
        }
        ("POST", "/__mock/calls") => write_append_calls(&mut stream, &state, &request.body).await,
        ("POST", _) => {
            let Some(route) = route(&request.path) else {
                return write_json(
                    &mut stream,
                    404,
                    &json!({"error": "unknown mock LLM endpoint"}),
                    &BTreeMap::new(),
                )
                .await;
            };
            handle_completion(&mut stream, request, &state, route.protocol, route.model).await
        }
        _ => {
            write_json(
                &mut stream,
                404,
                &json!({"error": "unknown mock LLM endpoint"}),
                &BTreeMap::new(),
            )
            .await
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AppendCallsRequest {
    calls: Vec<Value>,
}

async fn write_append_calls(
    stream: &mut TcpStream,
    state: &ServerState,
    body: &[u8],
) -> Result<()> {
    let request = match serde_json::from_slice::<AppendCallsRequest>(body) {
        Ok(request) => request,
        Err(error) => {
            return write_json(
                stream,
                400,
                &json!({"error": format!("invalid append request: {error}")}),
                &BTreeMap::new(),
            )
            .await;
        }
    };
    match state.append_calls(request.calls).await {
        Ok((appended, remaining)) => {
            write_json(
                stream,
                200,
                &json!({"appended": appended, "remaining": remaining}),
                &BTreeMap::new(),
            )
            .await
        }
        Err(error) => {
            write_json(
                stream,
                400,
                &json!({"error": error.to_string()}),
                &BTreeMap::new(),
            )
            .await
        }
    }
}

async fn write_models(stream: &mut TcpStream) -> Result<()> {
    write_json(
        stream,
        200,
        &json!({
            "object": "list", "data": [{
                "id": "claude-opus-4-8", "object": "model", "created": 0,
                "owned_by": "loopal-mock-llm", "supports_thinking": true,
                "supports_function_calling": true, "context_window": 200000
            }]
        }),
        &BTreeMap::new(),
    )
    .await
}
