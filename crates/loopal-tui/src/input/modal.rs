use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::InputAction;
use super::sub_page::handle_sub_page_key;
use crate::app::App;

#[derive(Default)]
struct ModalState {
    has_perm: bool,
    has_question: bool,
    on_other: bool,
    multi: bool,
}

fn read_modal_state(app: &App) -> ModalState {
    app.with_active_conversation(|conv| {
        let has_perm = conv.pending_permission.is_some();
        let (has_question, on_other, multi) = match conv.pending_question.as_ref() {
            Some(q) => (true, q.cursor_on_other(), q.allow_multiple_for_current()),
            None => (false, false, false),
        };
        ModalState {
            has_perm,
            has_question,
            on_other,
            multi,
        }
    })
}

pub(super) fn handle_modal_keys(app: &mut App, key: &KeyEvent) -> Option<InputAction> {
    let st = read_modal_state(app);
    let is_ctrl_c = key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c');
    if st.has_perm {
        return Some(match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => InputAction::ToolApprove,
            KeyCode::Char('n') | KeyCode::Char('N') => InputAction::ToolDeny,
            KeyCode::Esc => InputAction::ToolDeny,
            _ if is_ctrl_c => InputAction::ToolDeny,
            _ => InputAction::None,
        });
    }
    if st.has_question {
        return Some(question_action(key, &st, is_ctrl_c));
    }
    if app.sub_page.is_some() {
        return Some(handle_sub_page_key(app, key));
    }
    None
}

fn question_action(key: &KeyEvent, st: &ModalState, is_ctrl_c: bool) -> InputAction {
    if is_ctrl_c {
        return InputAction::QuestionCancel;
    }
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    if ctrl && key.code == KeyCode::Char('v') {
        return InputAction::PasteRequested;
    }
    match key.code {
        KeyCode::Up => InputAction::QuestionUp,
        KeyCode::Down => InputAction::QuestionDown,
        KeyCode::Enter => InputAction::QuestionConfirm,
        KeyCode::Esc => InputAction::QuestionCancel,
        KeyCode::Char(' ') if st.multi => InputAction::QuestionToggle,
        KeyCode::Backspace if st.on_other => InputAction::QuestionFreeTextBackspace,
        KeyCode::Delete if st.on_other => InputAction::QuestionFreeTextDelete,
        KeyCode::Left if st.on_other => InputAction::QuestionFreeTextCursorLeft,
        KeyCode::Right if st.on_other => InputAction::QuestionFreeTextCursorRight,
        KeyCode::Home if st.on_other => InputAction::QuestionFreeTextHome,
        KeyCode::End if st.on_other => InputAction::QuestionFreeTextEnd,
        KeyCode::Char(c) if st.on_other && !ctrl => InputAction::QuestionFreeTextChar(c),
        _ => InputAction::None,
    }
}
