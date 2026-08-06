use loopal_provider::{AnthropicProvider, GoogleProvider, OpenAiCompatProvider, OpenAiProvider};
use loopal_provider_api::{EffortLevel, Provider, StopReason, StreamChunk, ThinkingConfig};
use serde_json::json;

use super::helpers::{API_KEY, collect, semantic_call, start};

#[tokio::test]
async fn one_semantic_scenario_drives_every_real_provider() {
    let protocols = ["anthropic", "openai_responses", "openai_compat", "google"];
    let calls = protocols.map(semantic_call);
    let (base, task) = start(json!({
        "version": 2, "name": "multi-provider-wire", "calls": calls
    }))
    .await;
    let providers: Vec<(&str, Box<dyn Provider>, ThinkingConfig)> = vec![
        (
            "anthropic",
            Box::new(AnthropicProvider::new(API_KEY.into()).with_base_url(base.clone())),
            ThinkingConfig::Budget { tokens: 64 },
        ),
        (
            "openai_responses",
            Box::new(OpenAiProvider::new(API_KEY.into()).with_base_url(base.clone())),
            ThinkingConfig::Effort {
                level: EffortLevel::Medium,
            },
        ),
        (
            "openai_compat",
            Box::new(OpenAiCompatProvider::new(
                API_KEY.into(),
                base.clone(),
                "compat-contract".into(),
            )),
            ThinkingConfig::Effort {
                level: EffortLevel::Medium,
            },
        ),
        (
            "google",
            Box::new(GoogleProvider::new(API_KEY.into()).with_base_url(base.clone())),
            ThinkingConfig::Budget { tokens: 64 },
        ),
    ];

    for (protocol, provider, thinking) in providers {
        assert_semantics(protocol, &collect(provider.as_ref(), thinking).await);
    }
    assert_journal(&base, &protocols).await;
    task.abort();
}

fn assert_semantics(protocol: &str, chunks: &[StreamChunk]) {
    assert!(
        chunks
            .iter()
            .any(|chunk| matches!(chunk, StreamChunk::Text { text } if text == "hello wire"))
    );
    assert!(
        chunks.iter().any(
            |chunk| matches!(chunk, StreamChunk::Thinking { text } if text == "reasoning wire")
        )
    );
    assert!(chunks.iter().any(|chunk| matches!(
        chunk,
        StreamChunk::ToolUse { name, input, .. }
            if name == "Read" && input["file_path"] == "README.md"
    )));
    let usage = chunks.iter().fold((0, 0), |total, chunk| match chunk {
        StreamChunk::Usage {
            input_tokens,
            output_tokens,
            ..
        } => (total.0 + input_tokens, total.1 + output_tokens),
        _ => total,
    });
    assert_eq!(usage, (12, 7), "{protocol}: {chunks:#?}");
    assert!(chunks.iter().any(|chunk| matches!(
        chunk,
        StreamChunk::Done {
            stop_reason: StopReason::EndTurn
        }
    )));
    if matches!(protocol, "anthropic" | "openai_responses" | "google") {
        assert!(chunks.iter().any(|chunk| matches!(
            chunk,
            StreamChunk::ThinkingSignature { signature }
                if signature == "reasoning-signature"
        )));
    }
}

async fn assert_journal(base: &str, protocols: &[&str]) {
    let requests: serde_json::Value = reqwest::get(format!("{base}/__mock/requests"))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(requests.as_array().unwrap().len(), protocols.len());
    for (record, protocol) in requests.as_array().unwrap().iter().zip(protocols) {
        assert_eq!(record["protocol"], *protocol);
        assert_eq!(record["model"], "deepseek-reasoner");
        assert_eq!(record["messageCount"], 1);
        assert_eq!(record["toolCount"], 1);
        assert_eq!(record["lastUserText"], "wire contract marker");
        assert_eq!(record["hasSystem"], true);
        assert_eq!(record["stream"], true);
        assert_eq!(record["maxTokens"], 256);
        assert_eq!(record["apiKeyPresent"], true);
        assert_eq!(record["protocolVersionPresent"], true);
        assert_eq!(record["matched"], true);
    }
    let encoded = requests.to_string();
    assert!(!encoded.contains(API_KEY));
    let state: serde_json::Value = reqwest::get(format!("{base}/__mock/verify"))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(state["verified"], true);
}
