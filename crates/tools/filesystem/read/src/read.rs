use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult};
use serde_json::{Value, json};

pub struct ReadTool;

#[async_trait]
impl Tool for ReadTool {
    fn name(&self) -> &str {
        "Read"
    }

    fn description(&self) -> &str {
        "Reads a file from the local filesystem. You can access any file directly by using this tool.\n\
         Assume this tool is able to read all files on the machine. If a path is provided, assume it is valid. \
         It is okay to read a file that does not exist; an error will be returned.\n\n\
         Usage:\n\
         - The file_path parameter must be an absolute path, not a relative path.\n\
         - By default, it reads up to 2000 lines starting from the beginning of the file.\n\
         - You can optionally specify a line offset and limit (especially handy for long files), \
         but it's recommended to read the whole file by not providing these parameters.\n\
         - Results are returned using cat -n format, with line numbers starting at 1.\n\
         - This tool can read images (PNG, JPG, etc). When reading an image file the contents are presented visually.\n\
         - This tool can read Jupyter notebooks (.ipynb files) and returns all cells with their outputs.\n\
         - For PDF files, use the ReadPdf tool. For HTML files, use the ReadHtml tool.\n\
         - This tool can only read files, not directories. To read a directory, use Ls or an ls command via Bash.\n\
         - If the user provides a path to a screenshot, ALWAYS use this tool to view the file at the path.\n\
         - If you read a file that exists but has empty contents you will receive a system reminder warning."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["file_path"],
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Absolute path to the file to read"
                },
                "offset": {
                    "type": "integer",
                    "description": "Line number to start reading from (1-based)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of lines to read"
                }
            }
        })
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult, LoopalError> {
        let file_path = input["file_path"].as_str().ok_or_else(|| {
            LoopalError::Tool(loopal_error::ToolError::InvalidInput(
                "file_path is required".into(),
            ))
        })?;

        let offset = input["offset"].as_u64().unwrap_or(1).max(1) as usize;
        let limit = input["limit"].as_u64().unwrap_or(2000) as usize;

        match ctx.backend.read(file_path, offset - 1, limit).await {
            Ok(result) => Ok(ToolResult::success(result.content)),
            Err(e) => Ok(ToolResult::error(e.to_string())),
        }
    }
}
