use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::mpsc;

use loopal_memory::{MemoryObserver, MemoryProcessor};

/// Short batch window for tests.
const TEST_BATCH_WINDOW: Duration = Duration::from_millis(50);

/// Mock processor that records individual and batch calls separately.
struct BatchRecordingProcessor {
    single_calls: Mutex<Vec<String>>,
    batch_calls: Mutex<Vec<Vec<String>>>,
}

impl BatchRecordingProcessor {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            single_calls: Mutex::new(Vec::new()),
            batch_calls: Mutex::new(Vec::new()),
        })
    }
    fn single_calls(&self) -> Vec<String> {
        self.single_calls.lock().unwrap().clone()
    }
    fn batch_calls(&self) -> Vec<Vec<String>> {
        self.batch_calls.lock().unwrap().clone()
    }
    fn all_observations(&self) -> Vec<String> {
        let mut all = self.single_calls();
        for batch in self.batch_calls() {
            all.extend(batch);
        }
        all
    }
}

#[async_trait]
impl MemoryProcessor for BatchRecordingProcessor {
    async fn process(&self, observation: &str) -> Result<(), String> {
        self.single_calls
            .lock()
            .unwrap()
            .push(observation.to_string());
        Ok(())
    }

    async fn process_batch(&self, observations: &[String]) -> Result<(), String> {
        self.batch_calls
            .lock()
            .unwrap()
            .push(observations.to_vec());
        Ok(())
    }
}

/// Mock processor that records observations (single path only, uses default process_batch).
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

// ---------------------------------------------------------------------------
// Basic tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_observer_processes_each_observation() {
    let (tx, rx) = mpsc::channel(16);
    let processor = RecordingProcessor::new();
    let observer = MemoryObserver::new(rx, processor.clone(), TEST_BATCH_WINDOW);

    let handle = tokio::spawn(observer.run());

    tx.send("preference: bun".into()).await.unwrap();
    tx.send("convention: snake_case".into()).await.unwrap();
    drop(tx);

    handle.await.unwrap();
    let obs = processor.observations();
    assert!(obs.contains(&"preference: bun".to_string()));
    assert!(obs.contains(&"convention: snake_case".to_string()));
}

#[tokio::test]
async fn test_observer_stops_on_channel_close() {
    let (tx, rx) = mpsc::channel(16);
    let processor = RecordingProcessor::new();
    let observer = MemoryObserver::new(rx, processor, TEST_BATCH_WINDOW);

    let handle = tokio::spawn(observer.run());
    drop(tx);

    tokio::time::timeout(Duration::from_secs(1), handle)
        .await
        .expect("observer should exit within 1s")
        .unwrap();
}

#[tokio::test]
async fn test_observer_continues_on_processor_error() {
    let (tx, rx) = mpsc::channel(16);
    let observer = MemoryObserver::new(rx, Arc::new(FailingProcessor), TEST_BATCH_WINDOW);

    let handle = tokio::spawn(observer.run());

    tx.send("obs1".into()).await.unwrap();
    tx.send("obs2".into()).await.unwrap();
    tx.send("obs3".into()).await.unwrap();
    drop(tx);

    handle.await.unwrap();
}

// ---------------------------------------------------------------------------
// Batch processing tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_single_observation_calls_process_not_batch() {
    let (tx, rx) = mpsc::channel(16);
    let processor = BatchRecordingProcessor::new();
    let observer = MemoryObserver::new(rx, processor.clone(), TEST_BATCH_WINDOW);

    let handle = tokio::spawn(observer.run());

    tx.send("single fact".into()).await.unwrap();
    // Wait for the batch window to expire + processing
    tokio::time::sleep(Duration::from_millis(150)).await;
    drop(tx);

    handle.await.unwrap();
    assert_eq!(processor.single_calls(), vec!["single fact"]);
    assert!(processor.batch_calls().is_empty(), "process_batch should not be called for single observation");
}

#[tokio::test]
async fn test_multiple_observations_within_window_calls_batch() {
    let (tx, rx) = mpsc::channel(16);
    let processor = BatchRecordingProcessor::new();
    let observer = MemoryObserver::new(rx, processor.clone(), TEST_BATCH_WINDOW);

    let handle = tokio::spawn(observer.run());

    // Send multiple observations rapidly (within 50ms window)
    tx.send("obs1".into()).await.unwrap();
    tx.send("obs2".into()).await.unwrap();
    tx.send("obs3".into()).await.unwrap();
    // Wait for batch to be processed
    tokio::time::sleep(Duration::from_millis(150)).await;
    drop(tx);

    handle.await.unwrap();
    assert!(processor.single_calls().is_empty(), "process() should not be called for batch");
    let batches = processor.batch_calls();
    assert_eq!(batches.len(), 1, "should be exactly one batch");
    assert_eq!(batches[0], vec!["obs1", "obs2", "obs3"]);
}

#[tokio::test]
async fn test_observations_across_windows_produce_separate_batches() {
    let (tx, rx) = mpsc::channel(16);
    let processor = BatchRecordingProcessor::new();
    let observer = MemoryObserver::new(rx, processor.clone(), TEST_BATCH_WINDOW);

    let handle = tokio::spawn(observer.run());

    // First batch
    tx.send("batch1-obs1".into()).await.unwrap();
    tx.send("batch1-obs2".into()).await.unwrap();

    // Wait for first batch window to expire + processing
    tokio::time::sleep(Duration::from_millis(150)).await;

    // Second batch (separate window)
    tx.send("batch2-obs1".into()).await.unwrap();
    tokio::time::sleep(Duration::from_millis(150)).await;

    drop(tx);
    handle.await.unwrap();

    let all = processor.all_observations();
    assert!(all.contains(&"batch1-obs1".to_string()));
    assert!(all.contains(&"batch1-obs2".to_string()));
    assert!(all.contains(&"batch2-obs1".to_string()));
}

#[tokio::test]
async fn test_channel_close_during_batch_processes_remaining() {
    let (tx, rx) = mpsc::channel(16);
    let processor = BatchRecordingProcessor::new();
    let observer = MemoryObserver::new(rx, processor.clone(), Duration::from_secs(10)); // long window

    let handle = tokio::spawn(observer.run());

    tx.send("will-be-collected".into()).await.unwrap();
    tx.send("also-collected".into()).await.unwrap();
    // Close channel before window expires — remaining batch should still be processed
    drop(tx);

    handle.await.unwrap();

    let all = processor.all_observations();
    assert!(all.contains(&"will-be-collected".to_string()));
    assert!(all.contains(&"also-collected".to_string()));
}

#[tokio::test]
async fn test_default_process_batch_calls_process_sequentially() {
    let (tx, rx) = mpsc::channel(16);
    // RecordingProcessor does NOT override process_batch(), so default impl is used
    let processor = RecordingProcessor::new();
    let observer = MemoryObserver::new(rx, processor.clone(), TEST_BATCH_WINDOW);

    let handle = tokio::spawn(observer.run());

    tx.send("a".into()).await.unwrap();
    tx.send("b".into()).await.unwrap();
    tx.send("c".into()).await.unwrap();
    tokio::time::sleep(Duration::from_millis(150)).await;
    drop(tx);

    handle.await.unwrap();
    let obs = processor.observations();
    // Default process_batch calls process() for each, so all should appear
    assert_eq!(obs, vec!["a", "b", "c"]);
}

// ---------------------------------------------------------------------------
// Prompt embedding tests
// ---------------------------------------------------------------------------

#[test]
fn test_memory_agent_prompt_is_embedded() {
    let prompt = loopal_memory::MEMORY_AGENT_PROMPT;
    assert!(prompt.contains("Knowledge Manager Agent"));
    assert!(prompt.contains("MEMORY.md"));
    assert!(prompt.contains("executive summary"));
    assert!(prompt.contains("TTL Rules"));
}

#[test]
fn test_consolidation_prompt_is_embedded() {
    let prompt = loopal_memory::MEMORY_CONSOLIDATION_PROMPT;
    assert!(prompt.contains("Memory Consolidation Agent"));
    assert!(prompt.contains("memory dream"));
    assert!(prompt.contains("Staleness Check"));
    assert!(prompt.contains("Cross-Reference Integrity"));
}
