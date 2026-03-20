use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult};
use serde_json::{json, Value};

pub struct AskUserTool;

#[async_trait]
impl Tool for AskUserTool {
    fn name(&self) -> &str {
        "AskUser"
    }

    fn description(&self) -> &str {
        "Present one or more questions to the user with predefined options. \
         Use this when you need clarification or a decision from the user."
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

    async fn execute(&self, _input: Value, _ctx: &ToolContext) -> Result<ToolResult, LoopalError> {
        // Intercepted by the agent loop runner before reaching here.
        Ok(ToolResult::success("(intercepted by runner)"))
    }
}
