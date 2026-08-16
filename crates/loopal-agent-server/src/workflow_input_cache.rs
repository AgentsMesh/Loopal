use std::collections::{HashMap, VecDeque};

use loopal_runtime::workflow_input::WorkflowInputDisposition;

pub(super) struct DecisionCache {
    pub(super) values: HashMap<uuid::Uuid, WorkflowInputDisposition>,
    pub(super) insertion_order: VecDeque<uuid::Uuid>,
    capacity: usize,
}

impl DecisionCache {
    pub(super) fn new(capacity: usize) -> Self {
        Self {
            values: HashMap::with_capacity(capacity),
            insertion_order: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub(super) fn get(&self, envelope_id: &uuid::Uuid) -> Option<WorkflowInputDisposition> {
        self.values.get(envelope_id).copied()
    }

    pub(super) fn insert(
        &mut self,
        envelope_id: uuid::Uuid,
        disposition: WorkflowInputDisposition,
    ) {
        if self.capacity == 0 {
            return;
        }
        if let Some(current) = self.values.get_mut(&envelope_id) {
            *current = disposition;
            return;
        }
        while self.values.len() >= self.capacity {
            let Some(oldest) = self.insertion_order.pop_front() else {
                self.values.clear();
                break;
            };
            self.values.remove(&oldest);
        }
        self.insertion_order.push_back(envelope_id);
        self.values.insert(envelope_id, disposition);
    }
}
