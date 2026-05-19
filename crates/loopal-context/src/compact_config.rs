use std::time::Duration;

/// Why 80 (not 95): tokenizer drift between local cl100k_base and the
/// provider's tokenizer is around 15% on Anthropic payloads, so 80
/// leaves room for the request even when the local count underestimates.
pub const COMPACTION_RATIO_PERCENT: u32 = 80;

/// Sized for Haiku 4.5's 16K output cap; lower values would truncate
/// the 9-section summary.
pub const COMPACT_MAX_OUTPUT_TOKENS: u32 = 16_384;

/// Exponential backoff for retryable LLM failures (5xx, network). The
/// length of this slice determines the attempt count.
pub const RETRY_BACKOFF: &[Duration] = &[
    Duration::from_secs(1),
    Duration::from_secs(2),
    Duration::from_secs(4),
];

/// Why distinct from REHYDRATE_TOP_N: naming a file in the summary is
/// cheap; re-reading is not. The summary lists more files than we
/// re-read so the model can grep them deliberately.
pub const TOUCHED_FILES_HINT_LIMIT: usize = 10;

pub const REHYDRATE_TOP_N: usize = 5;

pub const REHYDRATE_PER_FILE_BYTES: usize = 5_000;

/// Roughly ~12K tokens — sized so the rehydrated tail fits inside one
/// cache-extending input block.
pub const REHYDRATE_TOTAL_BYTES: usize = 50_000;

/// Per-file cap on the `Read` tool invocation. Files that take longer
/// than this are skipped (not retried) so a hung filesystem call can't
/// block the whole turn.
pub const REHYDRATE_TIMEOUT: Duration = Duration::from_secs(30);

pub const LAYER1_TRUNCATE_MAX_LINES: usize = 200;
pub const LAYER1_TRUNCATE_MAX_BYTES: usize = 8_000;

/// Below this, the older results pay for themselves in context; above
/// it they outweigh their information value and get truncated.
pub const LAYER1_TRIGGER_PERCENT: u32 = 60;
