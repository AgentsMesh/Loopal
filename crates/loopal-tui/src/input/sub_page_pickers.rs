use crossterm::event::{KeyCode, KeyEvent};

use crate::app::{App, SubPage};

use super::sub_page::{PickerKeyResult, dismiss_picker, handle_generic_picker_key};
use super::{InputAction, SubPageResult};

pub(super) fn handle_enum_picker_key(app: &mut App, key: &KeyEvent) -> InputAction {
    let picker = match app.sub_page.as_mut().unwrap() {
        SubPage::EnumPicker { state, .. } => state,
        _ => unreachable!(),
    };
    match handle_generic_picker_key(picker, key) {
        PickerKeyResult::Dismiss => {
            dismiss_picker(app);
            return InputAction::None;
        }
        PickerKeyResult::Handled => return InputAction::None,
        PickerKeyResult::Unhandled => {}
    }
    let (picker, kind) = match app.sub_page.as_mut().unwrap() {
        SubPage::EnumPicker { state, kind } => (state, *kind),
        _ => unreachable!(),
    };
    if key.code != KeyCode::Enter {
        return InputAction::None;
    }
    let filtered = picker.filtered_items();
    if let Some(item) = filtered.get(picker.selected) {
        let value = item.value.clone();
        dismiss_picker(app);
        InputAction::SubPageConfirm(SubPageResult::EnumConfigSelected { kind, value })
    } else {
        app.sub_page = None;
        InputAction::None
    }
}
