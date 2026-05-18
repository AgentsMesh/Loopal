use crossterm::event::{KeyCode, KeyEvent};

use super::InputAction;
use crate::app::{App, SubPage};

pub(super) fn handle_cron_detail_key(app: &mut App, key: &KeyEvent) -> InputAction {
    if !matches!(app.sub_page, Some(SubPage::CronDetail(_))) {
        return InputAction::None;
    }
    match key.code {
        KeyCode::Esc => {
            app.sub_page = None;
            app.last_esc_time = None;
            InputAction::None
        }
        KeyCode::Char('x') | KeyCode::Char('X') => InputAction::StopFocusedSubPageItem,
        _ => InputAction::None,
    }
}
