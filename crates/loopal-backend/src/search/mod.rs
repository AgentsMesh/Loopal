//! Optimized glob and grep search with parallel traversal.

use std::path::PathBuf;

use loopal_error::ToolIoError;
use loopal_tool_api::backend_types::{
    GlobOptions, GlobSearchResult, GrepOptions, GrepSearchResult,
};

use crate::limits::ResourceLimits;

mod binary;
mod glob;
mod grep;
mod grep_file;
mod grep_match;
mod walker;

pub use glob::glob_search;
pub use grep::grep_search;

pub async fn glob_search_async(
    opts: GlobOptions,
    cwd: PathBuf,
    limits: ResourceLimits,
) -> Result<GlobSearchResult, ToolIoError> {
    tokio::task::spawn_blocking(move || glob_search(&opts, &cwd, &limits))
        .await
        .map_err(|e| ToolIoError::Other(e.to_string()))?
}

pub async fn grep_search_async(
    opts: GrepOptions,
    cwd: PathBuf,
    limits: ResourceLimits,
) -> Result<GrepSearchResult, ToolIoError> {
    tokio::task::spawn_blocking(move || grep_search(&opts, &cwd, &limits))
        .await
        .map_err(|e| ToolIoError::Other(e.to_string()))?
}
