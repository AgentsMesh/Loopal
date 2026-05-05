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
    async fn execute(&self, app: &mut App, _arg: Option<&str>) -> CommandEffect {
        open_model_picker(app);
        CommandEffect::Done
    }
}

fn open_model_picker(app: &mut App) {
    let active = app.session.lock().active_view.clone();
    let current_model = app.observable_for(&active).model;
    let current_thinking = app.session.lock().thinking_config.clone();

    let models = loopal_provider::list_all_models();
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

/// Build the 5 thinking options and determine which one is currently selected.
fn build_thinking_options(current: &str) -> (Vec<ThinkingOption>, usize) {
    let options = vec![
        ThinkingOption {
            label: "Auto",
            value: r#"{"type":"auto"}"#.to_string(),
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
            label: "Disabled",
            value: r#"{"type":"disabled"}"#.to_string(),
        },
    ];
    let idx = match current {
        "low" => 1,
        "medium" => 2,
        "high" => 3,
        "disabled" => 4,
        _ => 0, // "auto" or unknown
    };
    (options, idx)
}
