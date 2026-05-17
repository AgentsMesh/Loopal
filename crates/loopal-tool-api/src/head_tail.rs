use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct HeadTail {
    head: Mutex<Vec<String>>,
    tail: Mutex<VecDeque<String>>,
    head_max: usize,
    tail_max: usize,
    total_lines: AtomicUsize,
}

impl HeadTail {
    pub fn new(head_max: usize, tail_max: usize) -> Self {
        Self {
            head: Mutex::new(Vec::with_capacity(head_max)),
            tail: Mutex::new(VecDeque::with_capacity(tail_max + 1)),
            head_max,
            tail_max,
            total_lines: AtomicUsize::new(0),
        }
    }

    // reason: head fills first then sticks. Tail is a ring that always reflects
    // the most recent lines. Lines that hit between head_full and tail_visible
    // are dropped from observation but still count in total_lines, which is
    // how render_preview knows to insert the elided marker.
    pub fn push_line(&self, line: String) {
        let n = self.total_lines.fetch_add(1, Ordering::Relaxed);
        if n < self.head_max {
            self.head.lock().unwrap().push(line);
            return;
        }
        let mut tail = self.tail.lock().unwrap();
        if tail.len() >= self.tail_max {
            tail.pop_front();
        }
        tail.push_back(line);
    }

    pub fn total_lines(&self) -> usize {
        self.total_lines.load(Ordering::Relaxed)
    }

    pub fn was_truncated(&self) -> bool {
        let head_len = self.head.lock().unwrap().len();
        let tail_len = self.tail.lock().unwrap().len();
        self.total_lines() > head_len + tail_len
    }

    pub fn render_preview(&self) -> String {
        let head = self.head.lock().unwrap().clone();
        let tail: Vec<String> = self.tail.lock().unwrap().iter().cloned().collect();
        let total = self.total_lines();
        let visible = head.len() + tail.len();
        let elided = total.saturating_sub(visible);

        let mut out = Vec::with_capacity(head.len() + tail.len() + 1);
        out.extend(head);
        if elided > 0 {
            out.push(format!("[... {elided} lines elided ...]"));
        }
        out.extend(tail);
        out.join("\n")
    }
}
