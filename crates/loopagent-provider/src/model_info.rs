use loopagent_types::provider::ModelInfo;

struct ModelEntry {
    id: &'static str,
    provider: &'static str,
    display_name: &'static str,
    context_window: u32,
    max_output_tokens: u32,
    input_price_per_mtok: f64,
    output_price_per_mtok: f64,
}

impl ModelEntry {
    fn to_model_info(&self) -> ModelInfo {
        ModelInfo {
            id: self.id.to_string(),
            provider: self.provider.to_string(),
            display_name: self.display_name.to_string(),
            context_window: self.context_window,
            max_output_tokens: self.max_output_tokens,
            input_price_per_mtok: self.input_price_per_mtok,
            output_price_per_mtok: self.output_price_per_mtok,
        }
    }
}

static KNOWN_MODELS: &[ModelEntry] = &[
    ModelEntry {
        id: "claude-sonnet-4-20250514",
        provider: "anthropic",
        display_name: "Claude Sonnet 4",
        context_window: 200_000,
        max_output_tokens: 16_384,
        input_price_per_mtok: 3.0,
        output_price_per_mtok: 15.0,
    },
    ModelEntry {
        id: "claude-sonnet-4-6",
        provider: "anthropic",
        display_name: "Claude Sonnet 4.6",
        context_window: 200_000,
        max_output_tokens: 16_384,
        input_price_per_mtok: 3.0,
        output_price_per_mtok: 15.0,
    },
    ModelEntry {
        id: "claude-opus-4-20250514",
        provider: "anthropic",
        display_name: "Claude Opus 4",
        context_window: 200_000,
        max_output_tokens: 32_000,
        input_price_per_mtok: 15.0,
        output_price_per_mtok: 75.0,
    },
    ModelEntry {
        id: "claude-opus-4-6",
        provider: "anthropic",
        display_name: "Claude Opus 4.6",
        context_window: 200_000,
        max_output_tokens: 32_000,
        input_price_per_mtok: 15.0,
        output_price_per_mtok: 75.0,
    },
    ModelEntry {
        id: "claude-haiku-3-5-20241022",
        provider: "anthropic",
        display_name: "Claude 3.5 Haiku",
        context_window: 200_000,
        max_output_tokens: 8_192,
        input_price_per_mtok: 0.8,
        output_price_per_mtok: 4.0,
    },
    ModelEntry {
        id: "gpt-4o",
        provider: "openai",
        display_name: "GPT-4o",
        context_window: 128_000,
        max_output_tokens: 16_384,
        input_price_per_mtok: 2.5,
        output_price_per_mtok: 10.0,
    },
    ModelEntry {
        id: "gpt-4o-mini",
        provider: "openai",
        display_name: "GPT-4o Mini",
        context_window: 128_000,
        max_output_tokens: 16_384,
        input_price_per_mtok: 0.15,
        output_price_per_mtok: 0.6,
    },
    ModelEntry {
        id: "o3-mini",
        provider: "openai",
        display_name: "o3-mini",
        context_window: 200_000,
        max_output_tokens: 100_000,
        input_price_per_mtok: 1.1,
        output_price_per_mtok: 4.4,
    },
    ModelEntry {
        id: "gemini-2.0-flash",
        provider: "google",
        display_name: "Gemini 2.0 Flash",
        context_window: 1_000_000,
        max_output_tokens: 8_192,
        input_price_per_mtok: 0.075,
        output_price_per_mtok: 0.3,
    },
    ModelEntry {
        id: "gemini-2.5-pro-preview-05-06",
        provider: "google",
        display_name: "Gemini 2.5 Pro",
        context_window: 1_000_000,
        max_output_tokens: 65_536,
        input_price_per_mtok: 1.25,
        output_price_per_mtok: 10.0,
    },
];

/// Return metadata for all known models.
pub fn list_all_models() -> Vec<ModelInfo> {
    KNOWN_MODELS.iter().map(|m| m.to_model_info()).collect()
}

pub fn get_model_info(model_id: &str) -> Option<ModelInfo> {
    KNOWN_MODELS
        .iter()
        .find(|m| m.id == model_id)
        .map(|m| m.to_model_info())
}

/// Resolve provider name from model id prefix.
pub fn resolve_provider(model_id: &str) -> &'static str {
    if model_id.starts_with("claude") {
        "anthropic"
    } else if model_id.starts_with("gpt-")
        || model_id.starts_with("o1")
        || model_id.starts_with("o3")
    {
        "openai"
    } else if model_id.starts_with("gemini") {
        "google"
    } else {
        "openai_compat"
    }
}

