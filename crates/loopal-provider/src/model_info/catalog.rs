use loopal_provider_api::{CostTier, ModelInfo, QualityTier, SpeedTier, ThinkingCapability};

pub(crate) struct ModelEntry {
    pub id: &'static str,
    pub provider: &'static str,
    pub display_name: &'static str,
    pub context_window: u32,
    pub max_output_tokens: u32,
    pub thinking: ThinkingCapability,
    pub speed: SpeedTier,
    pub cost: CostTier,
    pub quality: QualityTier,
    pub supports_tools: bool,
    pub supports_vision: bool,
}

impl ModelEntry {
    pub fn to_model_info(&self) -> ModelInfo {
        ModelInfo {
            id: self.id.to_string(),
            provider: self.provider.to_string(),
            display_name: self.display_name.to_string(),
            context_window: self.context_window,
            max_output_tokens: self.max_output_tokens,
            thinking: self.thinking,
            speed: self.speed,
            cost: self.cost,
            quality: self.quality,
            supports_tools: self.supports_tools,
            supports_vision: self.supports_vision,
        }
    }
}

macro_rules! model {
    ($id:expr, $prov:expr, $name:expr, $ctx:expr, $out:expr, $think:ident,
     $spd:ident, $cost:ident, $qual:ident, $tools:expr, $vis:expr) => {
        ModelEntry {
            id: $id,
            provider: $prov,
            display_name: $name,
            context_window: $ctx,
            max_output_tokens: $out,
            thinking: ThinkingCapability::$think,
            speed: SpeedTier::$spd,
            cost: CostTier::$cost,
            quality: QualityTier::$qual,
            supports_tools: $tools,
            supports_vision: $vis,
        }
    };
}

// Abbreviations: T=ThinkingCapability, S=SpeedTier, C=CostTier, Q=QualityTier
pub(crate) static KNOWN_MODELS: &[ModelEntry] = &[
    // ── Anthropic ────────────────────────────────────────────────────
    model!(
        "claude-sonnet-4-20250514",
        "anthropic",
        "Claude Sonnet 4",
        200_000,
        64_000,
        BudgetRequired,
        Medium,
        Medium,
        Standard,
        true,
        true
    ),
    model!(
        "claude-sonnet-4-6",
        "anthropic",
        "Claude Sonnet 4.6",
        1_000_000,
        64_000,
        Adaptive,
        Medium,
        Medium,
        Standard,
        true,
        true
    ),
    model!(
        "claude-opus-4-20250514",
        "anthropic",
        "Claude Opus 4",
        200_000,
        32_000,
        BudgetRequired,
        Slow,
        High,
        Premium,
        true,
        true
    ),
    model!(
        "claude-opus-4-6",
        "anthropic",
        "Claude Opus 4.6",
        1_000_000,
        128_000,
        Adaptive,
        Slow,
        High,
        Premium,
        true,
        true
    ),
    model!(
        "claude-haiku-3-5-20241022",
        "anthropic",
        "Claude 3.5 Haiku",
        200_000,
        8_192,
        None,
        Fast,
        Low,
        Basic,
        true,
        true
    ),
    // ── OpenAI ───────────────────────────────────────────────────────
    model!(
        "gpt-4o", "openai", "GPT-4o", 128_000, 16_384, None, Medium, Medium, Standard, true, true
    ),
    model!(
        "gpt-4o-mini",
        "openai",
        "GPT-4o Mini",
        128_000,
        16_384,
        None,
        Fast,
        Low,
        Basic,
        true,
        true
    ),
    model!(
        "gpt-4.1", "openai", "GPT-4.1", 1_047_576, 32_768, None, Medium, Medium, Standard, true,
        true
    ),
    model!(
        "gpt-4.1-mini",
        "openai",
        "GPT-4.1 Mini",
        1_047_576,
        32_768,
        None,
        Fast,
        Low,
        Basic,
        true,
        true
    ),
    model!(
        "gpt-4.1-nano",
        "openai",
        "GPT-4.1 Nano",
        1_047_576,
        32_768,
        None,
        Fast,
        Low,
        Basic,
        true,
        true
    ),
    model!(
        "o3",
        "openai",
        "o3",
        200_000,
        100_000,
        ReasoningEffort,
        Slow,
        High,
        Premium,
        true,
        true
    ),
    model!(
        "o3-mini",
        "openai",
        "o3-mini",
        200_000,
        100_000,
        ReasoningEffort,
        Medium,
        Medium,
        Standard,
        true,
        false
    ),
    model!(
        "o4-mini",
        "openai",
        "o4-mini",
        200_000,
        100_000,
        ReasoningEffort,
        Medium,
        Medium,
        Standard,
        true,
        true
    ),
    // ── Google ───────────────────────────────────────────────────────
    model!(
        "gemini-2.0-flash",
        "google",
        "Gemini 2.0 Flash",
        1_000_000,
        8_192,
        None,
        Fast,
        Low,
        Basic,
        true,
        true
    ),
    model!(
        "gemini-2.5-pro-preview-05-06",
        "google",
        "Gemini 2.5 Pro",
        1_000_000,
        65_536,
        ThinkingBudget,
        Medium,
        Medium,
        Standard,
        true,
        true
    ),
    model!(
        "gemini-2.5-flash-preview-04-17",
        "google",
        "Gemini 2.5 Flash",
        1_000_000,
        65_536,
        ThinkingBudget,
        Fast,
        Low,
        Standard,
        true,
        true
    ),
    // ── OpenAI-compatible (DeepSeek) ─────────────────────────────────
    model!(
        "deepseek-chat",
        "openai_compat",
        "DeepSeek V3",
        128_000,
        8_192,
        None,
        Medium,
        Low,
        Standard,
        true,
        false
    ),
    model!(
        "deepseek-reasoner",
        "openai_compat",
        "DeepSeek R1",
        128_000,
        8_192,
        ReasoningEffort,
        Slow,
        Low,
        Standard,
        true,
        false
    ),
];
