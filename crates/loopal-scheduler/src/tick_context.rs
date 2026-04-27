//! Per-scheduler state bundled for `tick_loop`.
//!
//! Extracted from `tick.rs` so that file stays focused on the loop body
//! and stays within the project's 200-LOC budget. Every field here is
//! cloned from the corresponding `Arc` on `CronScheduler`.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use tokio::sync::{Mutex, RwLock, broadcast};

use crate::clock::Clock;
use crate::scheduler::ActiveBinding;
use crate::task::ScheduledTask;

pub(crate) struct TickContext {
    pub tasks: Arc<RwLock<Vec<ScheduledTask>>>,
    pub clock: Arc<dyn Clock>,
    pub active: Arc<Mutex<Option<ActiveBinding>>>,
    pub dirty: Arc<AtomicBool>,
    pub store_disabled: Arc<AtomicBool>,
    pub change_tx: broadcast::Sender<()>,
}
