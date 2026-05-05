use loopal_view_state::PendingQuestion;

use crate::app::App;

pub fn compute_question_answers(q: &PendingQuestion) -> Vec<String> {
    (0..q.questions.len())
        .map(|i| compute_answer_for(q, i))
        .collect()
}

fn compute_answer_for(q: &PendingQuestion, idx: usize) -> String {
    let Some(question) = q.questions.get(idx) else {
        return String::new();
    };
    let Some(state) = q.states.get(idx) else {
        return String::new();
    };
    let trimmed = state.free_text().trim();
    let on_other = state.cursor() == question.options.len();

    if !question.allow_multiple && on_other {
        return trimmed.to_string();
    }

    if question.allow_multiple {
        let mut parts: Vec<String> = question
            .options
            .iter()
            .zip(state.selection().iter())
            .filter(|(_, sel)| **sel)
            .map(|(o, _)| o.label.clone())
            .collect();
        if state.other_selected() && !trimmed.is_empty() {
            parts.push(trimmed.to_string());
        }
        return parts.join(", ");
    }

    if !state.interacted() && !question.options.is_empty() {
        return String::new();
    }
    question
        .options
        .get(state.cursor())
        .map(|opt| opt.label.clone())
        .unwrap_or_default()
}

fn with_question<F: FnOnce(&mut PendingQuestion)>(app: &mut App, f: F) {
    app.with_active_conversation_mut(|conv| {
        if let Some(ref mut q) = conv.pending_question {
            f(q);
        }
    });
}

pub(crate) fn cursor_up(app: &mut App) {
    with_question(app, |q| q.cursor_up());
}

pub(crate) fn cursor_down(app: &mut App) {
    with_question(app, |q| q.cursor_down());
}

pub(crate) fn toggle(app: &mut App) {
    with_question(app, |q| q.toggle());
}

pub(crate) fn free_text_char(app: &mut App, c: char) {
    with_question(app, |q| q.free_text_insert_char(c));
}

pub(crate) fn free_text_backspace(app: &mut App) {
    with_question(app, |q| q.free_text_backspace());
}

pub(crate) fn free_text_delete(app: &mut App) {
    with_question(app, |q| q.free_text_delete());
}

pub(crate) fn free_text_cursor_left(app: &mut App) {
    with_question(app, |q| q.free_text_cursor_left());
}

pub(crate) fn free_text_cursor_right(app: &mut App) {
    with_question(app, |q| q.free_text_cursor_right());
}

pub(crate) fn free_text_home(app: &mut App) {
    with_question(app, |q| q.free_text_cursor_home());
}

pub(crate) fn free_text_end(app: &mut App) {
    with_question(app, |q| q.free_text_cursor_end());
}

pub fn route_paste(app: &mut App, text: &str) -> bool {
    let on_other = app.with_active_conversation(|conv| {
        conv.pending_question
            .as_ref()
            .map(|q| q.cursor_on_other())
            .unwrap_or(false)
    });
    if !on_other {
        return false;
    }
    with_question(app, |q| {
        for c in text.chars() {
            if c == '\n' || c == '\r' {
                continue;
            }
            q.free_text_insert_char(c);
        }
    });
    true
}

/// Cancel the current question. Discards all per-question answers
/// (including any already-confirmed earlier questions in a multi-question
/// AskUser call) and signals `cancelled=true` to the LLM via the protocol.
pub(crate) async fn cancel(app: &mut App) {
    app.clear_transient_status();
    let pending = app.with_active_conversation_mut(|conv| conv.pending_question.take());
    if let Some(q) = pending {
        let agent = app.session.lock().active_view.clone();
        app.session.cancel_question(&agent, &q.id).await;
    }
}

pub(crate) async fn confirm(app: &mut App) {
    // 防止 silent confirmation：当前题未交互且有选项 且 cursor 不在 Other 行时，
    // 不提交答案，提示用户先用方向键选择或在 Other 行输入。
    let needs_interaction = app.with_active_conversation(|conv| {
        conv.pending_question
            .as_ref()
            .and_then(|q| {
                let cur = q.questions.get(q.current_question)?;
                let s = q.states.get(q.current_question)?;
                let cursor_on_other = s.cursor() == cur.options.len();
                Some(!s.interacted() && !cur.options.is_empty() && !cursor_on_other)
            })
            .unwrap_or(false)
    });
    if needs_interaction {
        app.set_transient_status("Press ↑/↓ to choose, or type into Other.");
        return;
    }

    let advanced = app.with_active_conversation_mut(|conv| {
        conv.pending_question
            .as_mut()
            .map(|q| q.advance_to_next())
            .unwrap_or(false)
    });
    if advanced {
        return;
    }
    app.clear_transient_status();
    let pending = app.with_active_conversation_mut(|conv| conv.pending_question.take());
    if let Some(q) = pending {
        let answers = compute_question_answers(&q);
        let agent = app.session.lock().active_view.clone();
        app.session.respond_question(&agent, &q.id, answers).await;
    }
}
