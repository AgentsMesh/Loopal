//! `/model` command — opens the model picker sub-page.

use async_trait::async_trait;

use super::{CommandEffect, CommandHandler};
use crate::app::{App, PickerItem, PickerState, SubPage, ThinkingOption};

pub struct ModelCmd;

#[async_trait]
impl CommandHandler for ModelCmd {
    fn name(&self) -> &str {
        "/model"
    }
    fn description(&self) -> &str {
        "Switch model"
    }
    fn has_arg(&self) -> bool {
        false
    }
    async fn execute(&self, app: &mut App, _arg: Option<&str>) -> CommandEffect {
        open_model_picker(app);
        CommandEffect::Done
    }
}

fn open_model_picker(app: &mut App) {
    let active = app.session.lock().active_view.clone();
    let observable = app.observable_for(&active);
    let current_model = observable.model.clone();
    let current_thinking = observable.thinking_config;

    let mut models = loopal_provider::list_all_models();
    if let Some(idx) = models.iter().position(|m| m.id == current_model) {
        let current = models.remove(idx);
        models.insert(0, current);
    }
    let items: Vec<PickerItem> = models
        .into_iter()
        .map(|m| {
            let marker = if m.id == current_model {
                " (current)"
            } else {
                ""
            };
            PickerItem {
                label: m.display_name.clone(),
                description: format!(
                    "{}  ctx:{}k  out:{}k{marker}",
                    m.id,
                    m.context_window / 1000,
                    m.max_output_tokens / 1000,
                ),
                value: m.id,
            }
        })
        .collect();

    let (thinking_options, thinking_selected) = build_thinking_options(&current_thinking);
    app.sub_page = Some(SubPage::ModelPicker(PickerState {
        title: "Switch Model".to_string(),
        items,
        filter: String::new(),
        filter_cursor: 0,
        selected: 0,
        thinking_options,
        thinking_selected,
    }));
}

/// Build thinking options and determine which one is currently selected.
fn build_thinking_options(current: &str) -> (Vec<ThinkingOption>, usize) {
    let options = vec![
        ThinkingOption {
            label: "Auto",
            value: r#"{"type":"auto"}"#.to_string(),
        },
        ThinkingOption {
            label: "None",
            value: r#"{"type":"effort","level":"none"}"#.to_string(),
        },
        ThinkingOption {
            label: "Low",
            value: r#"{"type":"effort","level":"low"}"#.to_string(),
        },
        ThinkingOption {
            label: "Medium",
            value: r#"{"type":"effort","level":"medium"}"#.to_string(),
        },
        ThinkingOption {
            label: "High",
            value: r#"{"type":"effort","level":"high"}"#.to_string(),
        },
        ThinkingOption {
            label: "XHigh",
            value: r#"{"type":"effort","level":"xhigh"}"#.to_string(),
        },
        ThinkingOption {
            label: "Max",
            value: r#"{"type":"effort","level":"max"}"#.to_string(),
        },
    ];
    let idx = match current {
        "none" => 1,
        "low" => 2,
        "medium" => 3,
        "high" => 4,
        "xhigh" => 5,
        "max" => 6,
        "disabled" => 1,
        _ => 0, // "auto" or unknown
    };
    (options, idx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thinking_options_have_stable_wire_values() {
        let (options, selected) = build_thinking_options("auto");
        let actual: Vec<(&str, &str)> = options
            .iter()
            .map(|option| (option.label, option.value.as_str()))
            .collect();
        assert_eq!(
            actual,
            vec![
                ("Auto", r#"{"type":"auto"}"#),
                ("None", r#"{"type":"effort","level":"none"}"#),
                ("Low", r#"{"type":"effort","level":"low"}"#),
                ("Medium", r#"{"type":"effort","level":"medium"}"#),
                ("High", r#"{"type":"effort","level":"high"}"#),
                ("XHigh", r#"{"type":"effort","level":"xhigh"}"#),
                ("Max", r#"{"type":"effort","level":"max"}"#),
            ]
        );
        assert_eq!(selected, 0);
    }

    #[test]
    fn thinking_selection_restores_new_and_legacy_values() {
        for (current, expected) in [("none", 1), ("xhigh", 5), ("max", 6), ("disabled", 1)] {
            assert_eq!(build_thinking_options(current).1, expected);
        }
    }
}
