use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use tokio::sync::mpsc;

use loopal_memory::{MemoryObserver, MemoryProcessor};

/// Mock processor that records observations it receives.
struct RecordingProcessor(Mutex<Vec<String>>);

impl RecordingProcessor {
    fn new() -> Arc<Self> {
        Arc::new(Self(Mutex::new(Vec::new())))
    }
    fn observations(&self) -> Vec<String> {
        self.0.lock().unwrap().clone()
    }
}

#[async_trait]
impl MemoryProcessor for RecordingProcessor {
    async fn process(&self, observation: &str) -> Result<(), String> {
        self.0.lock().unwrap().push(observation.to_string());
        Ok(())
    }
}

/// Mock processor that always fails.
struct FailingProcessor;

#[async_trait]
impl MemoryProcessor for FailingProcessor {
    async fn process(&self, _observation: &str) -> Result<(), String> {
        Err("simulated failure".into())
    }
}

#[tokio::test]
async fn test_observer_processes_each_observation() {
    let (tx, rx) = mpsc::channel(16);
    let processor = RecordingProcessor::new();
    let observer = MemoryObserver::new(rx, processor.clone());

    let handle = tokio::spawn(observer.run());

    tx.send("preference: bun".into()).await.unwrap();
    tx.send("convention: snake_case".into()).await.unwrap();
    drop(tx); // close channel → observer exits

    handle.await.unwrap();
    assert_eq!(processor.observations(), vec!["preference: bun", "convention: snake_case"]);
}

#[tokio::test]
async fn test_observer_stops_on_channel_close() {
    let (tx, rx) = mpsc::channel(16);
    let processor = RecordingProcessor::new();
    let observer = MemoryObserver::new(rx, processor);

    let handle = tokio::spawn(observer.run());
    drop(tx); // immediately close

    // Observer should exit promptly
    tokio::time::timeout(std::time::Duration::from_secs(1), handle)
        .await
        .expect("observer should exit within 1s")
        .unwrap();
}

#[tokio::test]
async fn test_observer_continues_on_processor_error() {
    let (tx, rx) = mpsc::channel(16);
    let observer = MemoryObserver::new(rx, Arc::new(FailingProcessor));

    let handle = tokio::spawn(observer.run());

    // Send multiple observations — observer should not crash
    tx.send("obs1".into()).await.unwrap();
    tx.send("obs2".into()).await.unwrap();
    tx.send("obs3".into()).await.unwrap();
    drop(tx);

    // Should complete without panic
    handle.await.unwrap();
}

#[test]
fn test_memory_agent_prompt_is_embedded() {
    let prompt = loopal_memory::MEMORY_AGENT_PROMPT;
    assert!(prompt.contains("Memory Maintenance Agent"));
    assert!(prompt.contains("MEMORY.md"));
    assert!(prompt.contains("AttemptCompletion"));
}
