use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, ToolContext, ToolResult, TypedTool};
use schemars::JsonSchema;
use serde::Deserialize;

pub struct ReadTool;

#[derive(Deserialize, JsonSchema)]
pub struct ReadParams {
    pub file_path: String,
    #[serde(default)]
    pub offset: Option<u64>,
    #[serde(default)]
    pub limit: Option<u64>,
}

#[async_trait]
impl TypedTool<ReadParams> for ReadTool {
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

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    async fn execute(
        &self,
        input: ReadParams,
        ctx: &ToolContext,
    ) -> Result<ToolResult, LoopalError> {
        let offset = input.offset.unwrap_or(1).max(1) as usize;
        let limit = input.limit.unwrap_or(2000) as usize;

        match ctx.backend.read(&input.file_path, offset - 1, limit).await {
            Ok(result) => Ok(ToolResult::success(result.content)),
            Err(e) => Ok(ToolResult::error(e.to_string())),
        }
    }
}
