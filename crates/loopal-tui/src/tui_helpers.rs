//! Helper functions extracted from the TUI main loop.

use loopal_protocol::{Envelope, MessageSource, UserContent};

use crate::app::App;

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

/// Route a human message to the primary agent via mailbox_tx.
pub async fn route_human_message(app: &App, content: UserContent) {
    let primary = app.session.primary();
    if let Some(ref tx) = primary.mailbox_tx {
        let envelope = Envelope::new(MessageSource::Human, "main", content);
        if let Err(e) = tx.send(envelope).await {
            tracing::warn!(error = %e, "failed to route human message");
        }
    } else {
        tracing::warn!("no mailbox_tx configured — message dropped");
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
