use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use tokio::sync::{Mutex, RwLock, broadcast, mpsc};

use crate::clock::Clock;
use crate::save_worker::SaveMessage;
use crate::scheduler::ActiveBinding;
use crate::task::ScheduledTask;

pub(crate) struct TickContext {
    pub tasks: Arc<RwLock<Vec<ScheduledTask>>>,
    pub clock: Arc<dyn Clock>,
    pub active: Arc<Mutex<Option<ActiveBinding>>>,
    pub dirty: Arc<AtomicBool>,
    pub store_disabled: Arc<AtomicBool>,
    pub change_tx: broadcast::Sender<()>,
    pub save_tx: mpsc::Sender<SaveMessage>,
}
