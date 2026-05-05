use crossterm::event::{self, Event as CrosstermEvent, KeyEvent, MouseEventKind};
use loopal_protocol::AgentEvent;
use tokio::sync::mpsc;

use crate::input::paste::PasteResult;

#[derive(Debug)]
pub enum AppEvent {
    Key(KeyEvent),
    ScrollUp,
    ScrollDown,
    Resize(u16, u16),
    Agent(AgentEvent),
    Paste(PasteResult),
    Tick,
    Resync,
}

/// Merges crossterm terminal events with agent events into a single stream.
pub struct EventHandler {
    rx: mpsc::Receiver<AppEvent>,
    tx: mpsc::Sender<AppEvent>,
}

impl EventHandler {
    pub fn new(
        mut agent_rx: mpsc::Receiver<AgentEvent>,
        mut resync_rx: mpsc::Receiver<()>,
    ) -> Self {
        // Use a large buffer so that agent events are never blocked by
        // slow UI rendering. The agent runtime sends events (Stream,
        // ToolCall, TokenUsage, …) via a bounded channel; if our
        // internal channel fills up the forwarding task blocks, which
        // blocks the agent-side `event_tx.send().await` — deadlock.
        let (tx, rx) = mpsc::channel(4096);

        // Spawn crossterm event polling task.
        //
        // Reads ALL buffered events in a single `spawn_blocking` call so
        // that rapid events (e.g. paste sequences) land in the channel
        // together, improving batch processing in `tui_loop.rs`.
        let term_tx = tx.clone();
        tokio::spawn(async move {
            loop {
                let result = tokio::task::spawn_blocking(|| {
                    // Wait up to 50ms for the first event.
                    if !event::poll(std::time::Duration::from_millis(50)).unwrap_or(false) {
                        return Vec::new();
                    }
                    let mut events = Vec::new();
                    if let Ok(ev) = event::read() {
                        events.push(ev);
                    }
                    // Drain any additional buffered events without waiting.
                    while event::poll(std::time::Duration::ZERO).unwrap_or(false) {
                        match event::read() {
                            Ok(ev) => events.push(ev),
                            Err(_) => break,
                        }
                    }
                    events
                })
                .await;

                match result {
                    Ok(events) if events.is_empty() => continue,
                    Ok(events) => {
                        for ev in events {
                            let app_event = match ev {
                                CrosstermEvent::Key(key) => Some(AppEvent::Key(key)),
                                CrosstermEvent::Mouse(mouse) => match mouse.kind {
                                    MouseEventKind::ScrollUp => Some(AppEvent::ScrollUp),
                                    MouseEventKind::ScrollDown => Some(AppEvent::ScrollDown),
                                    _ => None,
                                },
                                CrosstermEvent::Resize(w, h) => Some(AppEvent::Resize(w, h)),
                                CrosstermEvent::Paste(text) => {
                                    Some(AppEvent::Paste(PasteResult::Text(text)))
                                }
                                _ => None,
                            };
                            if let Some(app_event) = app_event
                                && term_tx.send(app_event).await.is_err()
                            {
                                return;
                            }
                        }
                    }
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

        let resync_fwd_tx = tx.clone();
        tokio::spawn(async move {
            while resync_rx.recv().await.is_some() {
                if resync_fwd_tx.send(AppEvent::Resync).await.is_err() {
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

    /// Create an EventHandler from a pre-built channel pair.
    ///
    /// No background tasks are spawned — the caller controls event injection
    /// through the returned `tx` handle. Used for server-mode E2E testing where
    /// crossterm polling is unavailable.
    pub fn from_channel(tx: mpsc::Sender<AppEvent>, rx: mpsc::Receiver<AppEvent>) -> Self {
        Self { rx, tx }
    }
}
