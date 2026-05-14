//! Monotonically increasing event ID + turn/correlation propagation.
//!
//! `next_event_id()` is process-global (envelope identity).
//!
//! `turn_id` / `correlation_id` flow via `tokio::task_local!`. Producers
//! enter `scope_turn` / `scope_correlation` at turn boundaries; nested
//! emit sites read the context via `TurnContext::try_current()`.
//!
//! Three accessors with explicit ergonomics:
//! - `TurnContext::try_current()` → `Option<TurnContext>`: caller MUST
//!   handle the "no scope" case (typical for envelope construction).
//! - `TurnContext::require_current()` → `TurnContext`: in-turn emit
//!   paths assert presence; **all build profiles `panic!`** if called
//!   outside scope (no silent fallback in release).
//! - `TurnContext::current_or_default()` → opt-in zero default for
//!   boundary emitters (cold start, shutdown).

use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_EVENT_ID: AtomicU64 = AtomicU64::new(1);

pub fn next_event_id() -> u64 {
    NEXT_EVENT_ID.fetch_add(1, Ordering::Relaxed)
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TurnContext {
    pub turn_id: u32,
    pub correlation_id: u64,
}

tokio::task_local! {
    static TURN_CTX: TurnContext;
}

impl TurnContext {
    /// Snapshot — `None` when called outside any `scope_turn`.
    pub fn try_current() -> Option<TurnContext> {
        TURN_CTX.try_with(|c| *c).ok()
    }

    /// Snapshot — falls back to default. Use from envelope constructors
    /// where "outside turn" is a legitimate state (cold start, shutdown).
    pub fn current_or_default() -> TurnContext {
        Self::try_current().unwrap_or_default()
    }

    /// Snapshot — panics (in both debug and release builds) if called
    /// outside `scope_turn`. Use from emit hot paths that MUST be in-turn;
    /// mis-scoped emit sites surface unambiguously in any build profile.
    pub fn require_current() -> TurnContext {
        Self::try_current().expect(
            "TurnContext::require_current called outside scope_turn — \
             this emit site MUST be inside an agent turn scope",
        )
    }
}

pub fn current_turn_id() -> u32 {
    TurnContext::current_or_default().turn_id
}

pub fn current_correlation_id() -> u64 {
    TurnContext::current_or_default().correlation_id
}

pub async fn scope_turn<F: Future>(turn_id: u32, f: F) -> F::Output {
    let ctx = TurnContext {
        turn_id,
        correlation_id: 0,
    };
    TURN_CTX.scope(ctx, f).await
}

pub async fn scope_correlation<F: Future>(correlation_id: u64, f: F) -> F::Output {
    let mut ctx = TurnContext::current_or_default();
    ctx.correlation_id = correlation_id;
    TURN_CTX.scope(ctx, f).await
}

/// Capture the current `TurnContext` and return a future that re-installs
/// it before running `f`. Use before `tokio::spawn` so spawned tasks
/// inherit the parent's turn/correlation IDs — otherwise `task_local!` is
/// reset to default at task boundary and emit sites would carry
/// `turn_id=0` silently.
///
/// Outside any scope, the captured context is the default (`turn_id=0`),
/// matching what `current_or_default` would produce — no scope is entered.
pub fn propagate_to_spawn<F>(f: F) -> impl Future<Output = F::Output>
where
    F: Future,
{
    let captured = TurnContext::try_current();
    async move {
        match captured {
            Some(ctx) => TURN_CTX.scope(ctx, f).await,
            None => f.await,
        }
    }
}
