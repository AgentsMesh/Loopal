//! Modal key handlers: tool confirm, question dialog, sub-page pickers.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::InputAction;
use super::sub_page::handle_sub_page_key;
use crate::app::App;

/// Handle modal states that override all other key bindings.
/// Returns `Some(action)` if a modal consumed the key, `None` to fall through.
pub(super) fn handle_modal_keys(app: &mut App, key: &KeyEvent) -> Option<InputAction> {
    if app
        .session
        .lock()
        .active_conversation()
        .pending_permission
        .is_some()
    {
        let is_ctrl_c =
            key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c');
        return Some(match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => InputAction::ToolApprove,
            KeyCode::Char('n') | KeyCode::Char('N') => InputAction::ToolDeny,
            KeyCode::Esc => InputAction::ToolDeny,
            _ if is_ctrl_c => InputAction::ToolDeny,
            _ => InputAction::None,
        });
    }
    if app
        .session
        .lock()
        .active_conversation()
        .pending_question
        .is_some()
    {
        let is_ctrl_c =
            key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c');
        return Some(match key.code {
            KeyCode::Up => InputAction::QuestionUp,
            KeyCode::Down => InputAction::QuestionDown,
            KeyCode::Enter => InputAction::QuestionConfirm,
            KeyCode::Char(' ') => InputAction::QuestionToggle,
            KeyCode::Esc => InputAction::QuestionCancel,
            _ if is_ctrl_c => InputAction::QuestionCancel,
            _ => InputAction::None,
        });
    }
    if app.sub_page.is_some() {
        return Some(handle_sub_page_key(app, key));
    }
    None
}
