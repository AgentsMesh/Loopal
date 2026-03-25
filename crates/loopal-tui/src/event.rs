use crossterm::event::{self, Event as CrosstermEvent, KeyEvent};
use loopal_protocol::AgentEvent;
use tokio::sync::mpsc;

use crate::input::paste::PasteResult;

/// Unified event type for the TUI main loop
#[derive(Debug)]
pub enum AppEvent {
    /// Keyboard / terminal event
    Key(KeyEvent),
    /// Resize event
    Resize(u16, u16),
    /// Agent event from the runtime
    Agent(AgentEvent),
    /// Clipboard paste completed (from background thread)
    Paste(PasteResult),
    /// Tick for periodic UI refresh
    Tick,
}

/// Merges crossterm terminal events with agent events into a single stream.
pub struct EventHandler {
    rx: mpsc::Receiver<AppEvent>,
    tx: mpsc::Sender<AppEvent>,
}

impl EventHandler {
    /// Create a new EventHandler.
    ///
    /// `agent_rx` receives AgentEvents from the runtime.
    /// Terminal events are polled in a background task.
    pub fn new(mut agent_rx: mpsc::Receiver<AgentEvent>) -> Self {
        // Use a large buffer so that agent events are never blocked by
        // slow UI rendering. The agent runtime sends events (Stream,
        // ToolCall, TokenUsage, …) via a bounded channel; if our
        // internal channel fills up the forwarding task blocks, which
        // blocks the agent-side `event_tx.send().await` — deadlock.
        let (tx, rx) = mpsc::channel(4096);

        // Spawn crossterm event polling task
        let term_tx = tx.clone();
        tokio::spawn(async move {
            loop {
                // Poll crossterm with a 50ms timeout to yield periodically
                match tokio::task::spawn_blocking(|| {
                    if event::poll(std::time::Duration::from_millis(50)).unwrap_or(false) {
                        event::read().ok()
                    } else {
                        None
                    }
                })
                .await
                {
                    Ok(Some(CrosstermEvent::Key(key))) => {
                        if term_tx.send(AppEvent::Key(key)).await.is_err() {
                            break;
                        }
                    }
                    Ok(Some(CrosstermEvent::Resize(w, h))) => {
                        if term_tx.send(AppEvent::Resize(w, h)).await.is_err() {
                            break;
                        }
                    }
                    Ok(Some(CrosstermEvent::Paste(text))) => {
                        let result = PasteResult::Text(text);
                        if term_tx.send(AppEvent::Paste(result)).await.is_err() {
                            break;
                        }
                    }
                    Ok(_) => {}
                    Err(_) => break,
                }
            }
        });

        // Spawn agent event forwarding task
        let agent_tx = tx.clone();
        tokio::spawn(async move {
            while let Some(event) = agent_rx.recv().await {
                if agent_tx.send(AppEvent::Agent(event)).await.is_err() {
                    break;
                }
            }
        });

        // Spawn tick task for periodic redraws.
        // Use try_send so ticks are dropped when the channel is busy
        // rather than blocking and causing back-pressure.
        let tick_tx = tx.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_millis(100));
            loop {
                interval.tick().await;
                // Drop tick if channel is full — cosmetic event, not critical
                if tick_tx.try_send(AppEvent::Tick).is_err() {
                    // Channel full or closed; if closed, exit
                    if tick_tx.is_closed() {
                        break;
                    }
                }
            }
        });

        Self { rx, tx }
    }

    /// Get a sender handle for injecting events (e.g. paste results).
    pub fn sender(&self) -> mpsc::Sender<AppEvent> {
        self.tx.clone()
    }

    /// Wait for the next event (blocking).
    pub async fn next(&mut self) -> Option<AppEvent> {
        self.rx.recv().await
    }

    /// Try to get the next event without waiting. Returns None if no event is ready.
    pub fn try_next(&mut self) -> Option<AppEvent> {
        self.rx.try_recv().ok()
    }
}
