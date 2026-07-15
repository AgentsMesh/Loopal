use serde::Serialize;
use serde_json::Value;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::Mutex;

use crate::{MockCall, RequestRecord, Scenario, WireProtocol};

pub(crate) struct ServerState {
    scenario: Mutex<Scenario>,
    requests: Mutex<Vec<RequestRecord>>,
    sequence: AtomicUsize,
    unmatched_requests: AtomicUsize,
    in_flight: AtomicUsize,
    client_disconnects: AtomicUsize,
    scripted_disconnects: AtomicUsize,
    api_key: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StateSnapshot {
    name: String,
    served: usize,
    remaining: usize,
    request_count: usize,
    unmatched_requests: usize,
    in_flight: usize,
    client_disconnects: usize,
    scripted_disconnects: usize,
    verified: bool,
}

impl ServerState {
    pub fn new(scenario: Scenario, api_key: String) -> Self {
        Self {
            scenario: Mutex::new(scenario),
            requests: Mutex::new(Vec::new()),
            sequence: AtomicUsize::new(0),
            unmatched_requests: AtomicUsize::new(0),
            in_flight: AtomicUsize::new(0),
            client_disconnects: AtomicUsize::new(0),
            scripted_disconnects: AtomicUsize::new(0),
            api_key,
        }
    }

    pub fn api_key(&self) -> &str {
        &self.api_key
    }

    pub async fn take_call(
        &self,
        protocol: WireProtocol,
        body: &Value,
        route_model: Option<&str>,
        key_present: bool,
        version_present: bool,
    ) -> (RequestRecord, Option<MockCall>) {
        let mut scenario = self.scenario.lock().await;
        let record = RequestRecord::from_body(
            self.sequence.fetch_add(1, Ordering::Relaxed) + 1,
            protocol,
            body,
            route_model,
            key_present,
            version_present,
        );
        let call = scenario.take_matching(body, &record);
        (record, call)
    }

    pub async fn record(&self, record: RequestRecord) {
        if !record.matched {
            self.unmatched_requests.fetch_add(1, Ordering::Relaxed);
        }
        let mut requests = self.requests.lock().await;
        if requests.len() == 1024 {
            requests.remove(0);
        }
        requests.push(record);
    }

    pub async fn requests(&self) -> Vec<RequestRecord> {
        self.requests.lock().await.clone()
    }

    pub fn record_client_disconnect(&self) {
        self.client_disconnects.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_scripted_disconnect(&self) {
        self.scripted_disconnects.fetch_add(1, Ordering::Relaxed);
    }

    pub fn begin_response(&self) -> ResponseGuard<'_> {
        self.in_flight.fetch_add(1, Ordering::Relaxed);
        ResponseGuard(self)
    }

    pub async fn snapshot(&self) -> StateSnapshot {
        let scenario = self.scenario.lock().await;
        let request_count = self.sequence.load(Ordering::Relaxed);
        let unmatched_requests = self.unmatched_requests.load(Ordering::Relaxed);
        let in_flight = self.in_flight.load(Ordering::Relaxed);
        StateSnapshot {
            name: scenario.name.clone(),
            served: scenario.served(),
            remaining: scenario.remaining(),
            request_count,
            unmatched_requests,
            in_flight,
            client_disconnects: self.client_disconnects.load(Ordering::Relaxed),
            scripted_disconnects: self.scripted_disconnects.load(Ordering::Relaxed),
            verified: scenario.remaining() == 0 && unmatched_requests == 0 && in_flight == 0,
        }
    }
}

pub(crate) struct ResponseGuard<'a>(&'a ServerState);

impl Drop for ResponseGuard<'_> {
    fn drop(&mut self) {
        self.0.in_flight.fetch_sub(1, Ordering::Relaxed);
    }
}
