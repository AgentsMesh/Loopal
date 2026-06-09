use crate::app::{App, EnumPickerKind, PickerItem, PickerState, SubPage};

pub(super) struct EnumOption {
    pub label: &'static str,
    pub description: &'static str,
    pub value: &'static str,
}

/// Effective value + whether it's a known live value. Empty (pre-cold-start)
/// falls back to `default_value` so the picker never highlights option 0 blindly
/// (which for sandbox is `disabled`).
fn effective_current<'a>(current: &'a str, default_value: &'a str) -> (&'a str, bool) {
    if current.is_empty() {
        (default_value, false)
    } else {
        (current, true)
    }
}

/// Effective value's option index, else the default's, else 0 — never blindly 0
/// when the default is a known option.
fn preselect_index(options: &[EnumOption], effective: &str, default_value: &str) -> usize {
    options
        .iter()
        .position(|o| o.value == effective)
        .or_else(|| options.iter().position(|o| o.value == default_value))
        .unwrap_or(0)
}

pub(super) fn open_enum_picker(
    app: &mut App,
    title: &str,
    kind: EnumPickerKind,
    current: &str,
    default_value: &str,
    options: &[EnumOption],
) {
    let (effective, known) = effective_current(current, default_value);
    let items: Vec<PickerItem> = options
        .iter()
        .map(|o| {
            let marker = if o.value == effective {
                if known { " (current)" } else { " (default)" }
            } else {
                ""
            };
            PickerItem {
                label: o.label.to_string(),
                description: format!("{}{marker}", o.description),
                value: o.value.to_string(),
            }
        })
        .collect();
    let selected = preselect_index(options, effective, default_value);
    app.sub_page = Some(SubPage::EnumPicker {
        state: PickerState {
            title: title.to_string(),
            items,
            filter: String::new(),
            filter_cursor: 0,
            selected,
            thinking_options: Vec::new(),
            thinking_selected: 0,
        },
        kind,
    });
}

#[cfg(test)]
mod tests {
    use super::{EnumOption, effective_current, preselect_index};

    fn opt(value: &'static str) -> EnumOption {
        EnumOption {
            label: value,
            description: value,
            value,
        }
    }

    #[test]
    fn known_live_value_is_used_and_marked_current() {
        assert_eq!(
            effective_current("read_only", "default_write"),
            ("read_only", true)
        );
    }

    #[test]
    fn empty_observable_falls_back_to_default_not_index_zero() {
        assert_eq!(
            effective_current("", "default_write"),
            ("default_write", false)
        );
    }

    #[test]
    fn preselect_uses_effective_when_present() {
        let options = [opt("disabled"), opt("default_write"), opt("read_only")];
        assert_eq!(preselect_index(&options, "read_only", "default_write"), 2);
    }

    #[test]
    fn preselect_unknown_value_falls_back_to_default_index_not_zero() {
        let options = [opt("disabled"), opt("default_write"), opt("read_only")];
        assert_eq!(
            preselect_index(&options, "workspace_write", "default_write"),
            1
        );
    }
}
