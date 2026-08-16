use std::collections::VecDeque;
use std::sync::Mutex;

struct TailState {
    lines: VecDeque<String>,
    snapshot: Option<String>,
}

pub struct OutputTail {
    state: Mutex<TailState>,
    max_lines: usize,
}

impl OutputTail {
    pub fn new(max_lines: usize) -> Self {
        Self {
            state: Mutex::new(TailState {
                lines: VecDeque::with_capacity(max_lines + 1),
                snapshot: None,
            }),
            max_lines,
        }
    }

    pub fn push_line(&self, line: String) {
        let mut state = self.state.lock().unwrap();
        state.snapshot = None;
        if self.max_lines == 0 {
            state.lines.clear();
            return;
        }
        if state.lines.len() >= self.max_lines {
            state.lines.pop_front();
        }
        state.lines.push_back(line);
    }

    pub fn replace_snapshot(&self, snapshot: String) {
        let mut state = self.state.lock().unwrap();
        state.lines.clear();
        state.snapshot = Some(snapshot);
    }

    pub fn max_lines(&self) -> usize {
        self.max_lines
    }

    pub fn snapshot(&self) -> String {
        let state = self.state.lock().unwrap();
        if let Some(snapshot) = &state.snapshot {
            return snapshot.clone();
        }
        state
            .lines
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join("\n")
    }
}
