use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolDispatch, ToolResult};
use serde_json::{Value, json};

pub struct AskUserTool;

#[async_trait]
impl Tool for AskUserTool {
    fn name(&self) -> &str {
        "AskUser"
    }

    fn description(&self) -> &str {
        "Present one or more questions to the user with predefined options.\n\
         Use when you need clarification, a decision, or user preferences.\n\
         - Users can always select 'Other' to provide custom text input.\n\
         - Use multiSelect: true when choices are not mutually exclusive.\n\
         - In plan mode: use this to clarify requirements BEFORE finalizing the plan. \
         Do NOT use it to ask 'Is my plan ready?' — use ExitPlanMode for that.\n\
         \n\
         Tool result format: per-question lines `Q{i} ({question}): {answer}` \
         joined by newline. Special tokens: `(cancelled by user)`, \
         `(no selection)`, `(unsupported: ...)` mean the user did not provide \
         a real answer."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["questions"],
            "properties": {
                "questions": {
                    "type": "array",
                    "description": "List of questions to present to the user",
                    "items": {
                        "type": "object",
                        "required": ["question", "options"],
                        "properties": {
                            "question": {
                                "type": "string",
                                "description": "The question text"
                            },
                            "header": {
                                "type": "string",
                                "description": "Short label displayed as a chip/tag (max 12 chars)"
                            },
                            "options": {
                                "type": "array",
                                "description": "Available answer options (2-4 items)",
                                "items": {
                                    "type": "object",
                                    "required": ["label", "description"],
                                    "properties": {
                                        "label": {
                                            "type": "string",
                                            "description": "Short label for the option"
                                        },
                                        "description": {
                                            "type": "string",
                                            "description": "Explanation of what this option means"
                                        }
                                    }
                                }
                            },
                            "multiSelect": {
                                "type": "boolean",
                                "description": "Allow selecting multiple options (default: false)"
                            }
                        }
                    }
                }
            }
        })
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    fn dispatch(&self) -> ToolDispatch {
        ToolDispatch::RunnerDirect
    }

    async fn execute(&self, _input: Value, _ctx: &ToolContext) -> Result<ToolResult, LoopalError> {
        // Intercepted by the agent loop runner before reaching here.
        Ok(ToolResult::success("(intercepted by runner)"))
    }
}
