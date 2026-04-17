//! Tests for agent_setup_helpers — pure helpers + async task spawn.

use std::sync::Arc;
use std::sync::Mutex;

use async_trait::async_trait;
use indexmap::IndexMap;
use loopal_agent_server::testing::{
    StartParams, build_initial_messages, collect_feature_tags, spawn_sub_agent_forwarder,
};
use loopal_config::{ResolvedConfig, Settings};
use loopal_error::Result;
use loopal_protocol::{AgentEvent, AgentEventPayload};
use loopal_runtime::frontend::traits::{AgentFrontend, EventEmitter};

fn empty_resolved_config() -> ResolvedConfig {
    ResolvedConfig {
        settings: Settings::default(),
        mcp_servers: IndexMap::new(),
        skills: IndexMap::new(),
        hooks: Vec::new(),
        instructions: String::new(),
        memory: String::new(),
        layers: Vec::new(),
    }
}

struct CaptureEmitter {
    events: Arc<Mutex<Vec<AgentEventPayload>>>,
}

#[async_trait]
impl EventEmitter for CaptureEmitter {
    async fn emit(&self, payload: AgentEventPayload) -> Result<()> {
        self.events.lock().unwrap().push(payload);
        Ok(())
    }
}

struct CaptureFrontend {
    events: Arc<Mutex<Vec<AgentEventPayload>>>,
}

#[async_trait]
impl AgentFrontend for CaptureFrontend {
    async fn emit(&self, payload: AgentEventPayload) -> Result<()> {
        self.events.lock().unwrap().push(payload);
        Ok(())
    }
    async fn recv_input(&self) -> Option<loopal_runtime::agent_input::AgentInput> {
        None
    }
    async fn request_permission(
        &self,
        _id: &str,
        _name: &str,
        _input: &serde_json::Value,
    ) -> loopal_tool_api::PermissionDecision {
        loopal_tool_api::PermissionDecision::Allow
    }
    fn event_emitter(&self) -> Box<dyn EventEmitter> {
        Box::new(CaptureEmitter {
            events: self.events.clone(),
        })
    }
}

fn blank_start() -> StartParams {
    StartParams {
        cwd: None,
        model: None,
        mode: None,
        prompt: None,
        permission_mode: None,
        no_sandbox: false,
        resume: None,
        lifecycle: loopal_runtime::LifecycleMode::Ephemeral,
        agent_type: None,
        depth: None,
        fork_context: None,
    }
}

#[test]
fn build_initial_messages_returns_resume_only_when_no_prompt_or_fork() {
    let resume = vec![loopal_message::Message::user("resumed A")];
    let out = build_initial_messages(resume.clone(), &blank_start());
    assert_eq!(out.len(), 1);
}

#[test]
fn build_initial_messages_appends_prompt_without_fork() {
    let mut start = blank_start();
    start.prompt = Some("do the thing".into());
    let out = build_initial_messages(Vec::new(), &start);
    assert_eq!(out.len(), 1);
}

#[test]
fn build_initial_messages_fork_added_when_no_resume() {
    let mut start = blank_start();
    let fork_msgs = vec![loopal_message::Message::user("fork msg 1")];
    start.fork_context = Some(serde_json::to_value(&fork_msgs).unwrap());
    start.prompt = Some("continue".into());
    let out = build_initial_messages(Vec::new(), &start);
    assert_eq!(out.len(), 2, "fork msgs + prompt");
}

#[test]
fn build_initial_messages_fork_ignored_when_resuming() {
    let mut start = blank_start();
    start.resume = Some("sid".into());
    let fork_msgs = vec![loopal_message::Message::user("fork msg")];
    start.fork_context = Some(serde_json::to_value(&fork_msgs).unwrap());
    let resume = vec![loopal_message::Message::user("resumed")];
    let out = build_initial_messages(resume, &start);
    assert_eq!(out.len(), 1, "fork ignored during resume");
}

#[test]
fn build_initial_messages_fork_json_fail_is_skipped() {
    let mut start = blank_start();
    start.fork_context = Some(serde_json::json!({"not_a_message_array": true}));
    start.prompt = Some("ok".into());
    let out = build_initial_messages(Vec::new(), &start);
    // Bad fork JSON is logged and skipped; prompt still appended.
    assert_eq!(out.len(), 1);
}

#[test]
fn collect_feature_tags_has_subagent_by_default() {
    let config = empty_resolved_config();
    let tags = collect_feature_tags(&config, false);
    assert!(tags.contains(&"subagent".into()));
}

#[test]
fn collect_feature_tags_memory_requires_channel_and_setting() {
    let mut config = empty_resolved_config();
    config.settings.memory.enabled = true;
    assert!(!collect_feature_tags(&config, false).contains(&"memory".into()));
    assert!(collect_feature_tags(&config, true).contains(&"memory".into()));
}

#[test]
fn collect_feature_tags_includes_output_style_when_set() {
    let mut config = empty_resolved_config();
    config.settings.output_style = "engineer".into();
    let tags = collect_feature_tags(&config, false);
    assert!(tags.iter().any(|t| t == "style_engineer"));
}

#[test]
fn collect_feature_tags_includes_hooks_when_settings_has_any() {
    use loopal_config::{HookConfig, HookEvent, HookType};
    use std::collections::HashMap;
    let mut config = empty_resolved_config();
    config.settings.hooks.push(HookConfig {
        event: HookEvent::PreToolUse,
        hook_type: HookType::Command,
        command: "echo".into(),
        url: None,
        headers: HashMap::new(),
        prompt: None,
        model: None,
        tool_filter: None,
        condition: None,
        timeout_ms: 10_000,
        id: None,
    });
    let tags = collect_feature_tags(&config, false);
    assert!(tags.contains(&"hooks".into()));
}

#[tokio::test]
async fn sub_agent_forwarder_emits_spawned_events_only() {
    let events = Arc::new(Mutex::new(Vec::<AgentEventPayload>::new()));
    let frontend = Arc::new(CaptureFrontend {
        events: events.clone(),
    });
    let tx = spawn_sub_agent_forwarder(frontend);
    tx.send(AgentEvent::root(AgentEventPayload::SubAgentSpawned {
        name: "child".into(),
        agent_id: "aid".into(),
        parent: None,
        model: None,
        session_id: None,
    }))
    .await
    .unwrap();
    tx.send(AgentEvent::root(AgentEventPayload::Started))
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let captured = events.lock().unwrap();
    assert_eq!(captured.len(), 1);
    assert!(matches!(
        captured[0],
        AgentEventPayload::SubAgentSpawned { .. }
    ));
}

#[tokio::test]
async fn sub_agent_forwarder_task_shuts_down_when_sender_dropped() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let frontend = Arc::new(CaptureFrontend {
        events: events.clone(),
    });
    {
        let _tx = spawn_sub_agent_forwarder(frontend);
        // _tx dropped at end of scope → channel closes → spawned task returns.
    }
    // If the task was not cleaned up, tokio runtime would still hold it;
    // we don't have a direct handle, but we can assert the test completes
    // without hanging and no emits happened.
    tokio::time::sleep(std::time::Duration::from_millis(30)).await;
    assert!(events.lock().unwrap().is_empty());
}
