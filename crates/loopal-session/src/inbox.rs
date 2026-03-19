/// Inbox queue for buffering user messages when the agent is busy.
use std::collections::VecDeque;

pub struct Inbox {
    queue: VecDeque<String>,
}

impl Inbox {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
        }
    }

    pub fn push(&mut self, text: String) {
        self.queue.push_back(text);
    }

    pub fn pop_front(&mut self) -> Option<String> {
        self.queue.pop_front()
    }

    pub fn pop_back(&mut self) -> Option<String> {
        self.queue.pop_back()
    }

    pub fn clear(&mut self) {
        self.queue.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = &String> {
        self.queue.iter()
    }
}

impl Default for Inbox {
    fn default() -> Self {
        Self::new()
    }
}
