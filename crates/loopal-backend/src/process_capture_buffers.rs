use std::collections::VecDeque;
use std::fmt::Write;

use zeroize::Zeroizing;

const STDERR_CAP_BYTES: usize = 8 * 1024;

pub(crate) struct HeadTail {
    head: Vec<Zeroizing<String>>,
    tail: VecDeque<Zeroizing<String>>,
    total: usize,
    head_max: usize,
    tail_max: usize,
}

impl HeadTail {
    pub fn new(head_max: usize, tail_max: usize) -> Self {
        Self {
            head: Vec::with_capacity(head_max),
            tail: VecDeque::with_capacity(tail_max + 1),
            total: 0,
            head_max,
            tail_max,
        }
    }

    pub fn push(&mut self, line: Zeroizing<String>) {
        self.total += 1;
        if self.head.len() < self.head_max {
            self.head.push(line);
        } else {
            if self.tail.len() >= self.tail_max {
                self.tail.pop_front();
            }
            self.tail.push_back(line);
        }
    }

    pub fn was_truncated(&self) -> bool {
        self.total > self.head.len() + self.tail.len()
    }

    pub fn render(&self) -> Zeroizing<String> {
        let mut output = Zeroizing::new(String::new());
        let mut needs_separator = false;
        for line in &self.head {
            append_line(&mut output, &mut needs_separator, line);
        }
        let elided = self.total.saturating_sub(self.head.len() + self.tail.len());
        if elided > 0 {
            if needs_separator {
                output.push('\n');
            }
            let _ = write!(&mut *output, "[... {elided} lines elided ...]");
            needs_separator = true;
        }
        for line in &self.tail {
            append_line(&mut output, &mut needs_separator, line);
        }
        output
    }
}

fn append_line(output: &mut String, needs_separator: &mut bool, line: &str) {
    if *needs_separator {
        output.push('\n');
    }
    output.push_str(line);
    *needs_separator = true;
}

pub(crate) struct Tail {
    lines: VecDeque<Zeroizing<String>>,
    max: usize,
}

impl Tail {
    pub fn new(max: usize) -> Self {
        Self {
            lines: VecDeque::with_capacity(max + 1),
            max,
        }
    }

    pub fn push(&mut self, line: Zeroizing<String>) {
        if self.max == 0 {
            return;
        }
        if self.lines.len() >= self.max {
            self.lines.pop_front();
        }
        self.lines.push_back(line);
    }

    pub fn render(&self) -> Zeroizing<String> {
        let mut output = Zeroizing::new(String::new());
        let mut needs_separator = false;
        for line in &self.lines {
            append_line(&mut output, &mut needs_separator, line);
        }
        output
    }
}

pub(crate) struct CappedText {
    text: Zeroizing<String>,
    pub truncated: bool,
}

impl CappedText {
    pub fn new() -> Self {
        Self {
            text: Zeroizing::new(String::new()),
            truncated: false,
        }
    }

    pub fn push(&mut self, text: &str) {
        self.text.push_str(text);
        if self.text.len() > STDERR_CAP_BYTES + 1024 {
            let mut split = self.text.len() - STDERR_CAP_BYTES;
            while !self.text.is_char_boundary(split) {
                split += 1;
            }
            self.text.drain(..split);
            self.truncated = true;
        }
    }

    pub fn snapshot(&self) -> Zeroizing<String> {
        let mut copy = Zeroizing::new(String::with_capacity(self.text.len()));
        copy.push_str(&self.text);
        copy
    }
}
