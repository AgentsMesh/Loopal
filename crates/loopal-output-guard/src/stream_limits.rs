use thiserror::Error;

pub const MAX_STREAM_SECRET_BYTES: usize = 65_536;
pub const MAX_STREAM_SECRET_NAME_BYTES: usize = 256;
pub const MAX_STREAM_SECRET_PATTERNS: usize = 128;
pub const MAX_STREAM_SECRET_TOTAL_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("secret set exceeds streaming redaction limits")]
pub struct StreamingOutputGuardBuildError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("streaming output guard is already finished")]
pub struct StreamingOutputGuardFinished;
