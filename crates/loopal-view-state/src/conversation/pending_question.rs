use loopal_protocol::Question;
use serde::{Deserialize, Serialize};

use super::question_state::QuestionState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingQuestion {
    pub id: String,
    pub questions: Vec<Question>,
    #[serde(default)]
    pub states: Vec<QuestionState>,
    pub current_question: usize,
}

impl PendingQuestion {
    pub fn new(id: String, questions: Vec<Question>) -> Self {
        let states = questions
            .iter()
            .map(|q| QuestionState::new(q.options.len()))
            .collect();
        Self {
            id,
            questions,
            states,
            current_question: 0,
        }
    }

    fn current(&self) -> Option<(&Question, &QuestionState)> {
        let q = self.questions.get(self.current_question)?;
        let s = self.states.get(self.current_question)?;
        Some((q, s))
    }

    fn current_mut(&mut self) -> Option<&mut QuestionState> {
        self.states.get_mut(self.current_question)
    }

    pub fn cursor(&self) -> usize {
        self.current().map(|(_, s)| s.cursor()).unwrap_or(0)
    }

    pub fn other_index(&self) -> usize {
        self.current().map(|(q, _)| q.options.len()).unwrap_or(0)
    }

    pub fn cursor_on_other(&self) -> bool {
        self.cursor() == self.other_index()
    }

    pub fn allow_multiple_for_current(&self) -> bool {
        self.current()
            .map(|(q, _)| q.allow_multiple)
            .unwrap_or(false)
    }

    pub fn other_is_selected(&self) -> bool {
        self.current()
            .map(|(_, s)| s.other_selected())
            .unwrap_or(false)
    }

    pub fn free_text(&self) -> &str {
        self.current().map(|(_, s)| s.free_text()).unwrap_or("")
    }

    pub fn free_text_cursor(&self) -> usize {
        self.current()
            .map(|(_, s)| s.free_text_cursor())
            .unwrap_or(0)
    }

    pub fn selection(&self) -> &[bool] {
        self.current().map(|(_, s)| s.selection()).unwrap_or(&[])
    }

    pub fn interacted(&self) -> bool {
        self.current().map(|(_, s)| s.interacted()).unwrap_or(false)
    }

    pub fn cursor_up(&mut self) {
        if let Some(s) = self.current_mut() {
            let next = s.cursor().saturating_sub(1);
            s.user_set_cursor_clamped(next, usize::MAX);
        }
    }

    pub fn cursor_down(&mut self) {
        let max = self.other_index();
        if let Some(s) = self.current_mut() {
            let next = s.cursor().saturating_add(1);
            s.user_set_cursor_clamped(next, max);
        }
    }

    pub fn toggle(&mut self) {
        if !self.allow_multiple_for_current() {
            return;
        }
        let on_other = self.cursor_on_other();
        let Some(s) = self.current_mut() else {
            return;
        };
        if on_other {
            s.toggle_other();
        } else {
            s.toggle_selection();
        }
    }

    pub fn get_answers(&self) -> Vec<String> {
        let Some((q, s)) = self.current() else {
            return Vec::new();
        };
        q.options
            .iter()
            .zip(s.selection().iter())
            .filter(|(_, sel)| **sel)
            .map(|(o, _)| o.label.clone())
            .collect()
    }

    pub fn advance_to_next(&mut self) -> bool {
        if self.current_question + 1 < self.questions.len() {
            self.current_question += 1;
            true
        } else {
            false
        }
    }

    pub fn free_text_insert_char(&mut self, c: char) {
        if let Some(s) = self.current_mut() {
            s.insert_char(c);
        }
    }

    pub fn free_text_backspace(&mut self) {
        if let Some(s) = self.current_mut() {
            s.backspace();
        }
    }

    pub fn free_text_delete(&mut self) {
        if let Some(s) = self.current_mut() {
            s.delete();
        }
    }

    pub fn free_text_cursor_left(&mut self) {
        if let Some(s) = self.current_mut() {
            s.cursor_left();
        }
    }

    pub fn free_text_cursor_right(&mut self) {
        if let Some(s) = self.current_mut() {
            s.cursor_right();
        }
    }

    pub fn free_text_cursor_home(&mut self) {
        if let Some(s) = self.current_mut() {
            s.cursor_home();
        }
    }

    pub fn free_text_cursor_end(&mut self) {
        if let Some(s) = self.current_mut() {
            s.cursor_end();
        }
    }
}
