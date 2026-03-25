//! Helper functions extracted from the TUI main loop.

use std::sync::Arc;

use loopal_agent::router::MessageRouter;
use loopal_protocol::{Envelope, MessageSource, UserContent};

use crate::app::{App, SubPage};

/// Handle mouse scroll events. Positive delta = scroll up, negative = down.
pub fn handle_scroll(app: &mut App, delta: i16) {
    // Sub-page picker: scroll moves selection
    if let Some(ref mut sub) = app.sub_page {
        match sub {
            SubPage::ModelPicker(p) => {
                if delta > 0 {
                    p.selected = p.selected.saturating_sub(1);
                } else {
                    let count = p.filtered_items().len();
                    if p.selected + 1 < count {
                        p.selected += 1;
                    }
                }
            }
            SubPage::RewindPicker(s) => {
                if delta > 0 {
                    s.selected = s.selected.saturating_sub(1);
                } else if s.selected + 1 < s.turns.len() {
                    s.selected += 1;
                }
            }
        }
        return;
    }
    // Question dialog: scroll moves cursor
    {
        let mut state = app.session.lock();
        if let Some(ref mut q) = state.pending_question {
            if delta > 0 {
                q.cursor_up();
            } else {
                q.cursor_down();
            }
            return;
        }
    }
    // Content area scroll
    let abs = delta.unsigned_abs();
    if delta > 0 {
        app.scroll_offset = app.scroll_offset.saturating_add(abs);
    } else {
        app.scroll_offset = app.scroll_offset.saturating_sub(abs);
    }
}

pub async fn handle_question_confirm(app: &mut App) {
    let answers = {
        let state = app.session.lock();
        state.pending_question.as_ref().map(|q| {
            let answers = q.get_answers();
            if answers.is_empty() && !q.questions[q.current_question].allow_multiple {
                let opt = &q.questions[q.current_question].options[q.cursor];
                vec![opt.label.clone()]
            } else {
                answers
            }
        })
    };
    if let Some(answers) = answers {
        app.session.answer_question(answers).await;
    }
}

/// Route a human message through the data plane.
pub async fn route_human_message(router: &Arc<MessageRouter>, target: &str, content: UserContent) {
    let envelope = Envelope::new(MessageSource::Human, target, content);
    if let Err(e) = router.route(envelope).await {
        tracing::warn!(error = %e, "failed to route human message — agent may have exited");
    }
}

/// Cycle focused_agent to the next agent in the agents map.
pub fn cycle_focus(app: &App) {
    let mut state = app.session.lock();
    let keys: Vec<String> = state.agents.keys().cloned().collect();
    if keys.is_empty() {
        state.focused_agent = None;
        return;
    }
    let next = match &state.focused_agent {
        None => keys[0].clone(),
        Some(current) => {
            let pos = keys.iter().position(|k| k == current);
            match pos {
                Some(i) if i + 1 < keys.len() => keys[i + 1].clone(),
                _ => keys[0].clone(),
            }
        }
    };
    state.focused_agent = Some(next);
}
