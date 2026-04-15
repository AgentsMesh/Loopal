use std::fs;
use std::thread::sleep;
use std::time::Duration;

use loopal_context::middleware::config_refresh::ConfigRefreshMiddleware;
use loopal_context::middleware::file_snapshot::FileSnapshot;
use loopal_message::{Message, MessageRole};
use loopal_provider_api::{Middleware, MiddlewareContext};

fn wait_for_mtime() {
    sleep(Duration::from_millis(1100));
}

fn make_ctx(messages: Vec<Message>) -> MiddlewareContext {
    MiddlewareContext {
        messages,
        system_prompt: "test".to_string(),
        model: "test-model".to_string(),
        total_input_tokens: 0,
        total_output_tokens: 0,
        max_context_tokens: 200_000,
        summarization_provider: None,
    }
}

#[tokio::test]
async fn no_change_no_injection() {
    let dir = std::env::temp_dir().join("loopal_cr_nochange_v1");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("mem.md");
    fs::write(&path, "stable content").unwrap();

    let snap = FileSnapshot::load(path, "Test");
    let mw = ConfigRefreshMiddleware::new(vec![snap]);
    let mut ctx = make_ctx(vec![Message::user("hello")]);

    mw.process(&mut ctx).await.unwrap();
    assert_eq!(ctx.messages.len(), 1);

    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn change_injects_reminder() {
    let dir = std::env::temp_dir().join("loopal_cr_change_v1");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("mem.md");
    fs::write(&path, "original").unwrap();

    let snap = FileSnapshot::load(path.clone(), "Project Memory");
    let mw = ConfigRefreshMiddleware::new(vec![snap]);

    wait_for_mtime();
    fs::write(&path, "updated line").unwrap();

    let mut ctx = make_ctx(vec![Message::user("hello")]);
    mw.process(&mut ctx).await.unwrap();

    assert_eq!(ctx.messages.len(), 2);
    let injected = &ctx.messages[1];
    assert_eq!(
        injected.role,
        MessageRole::User,
        "must be User to preserve prefix cache"
    );
    let text = injected.text_content();
    assert!(text.contains("system-reminder"));
    assert!(text.contains("Project Memory"));
    assert!(text.contains("updated line"));

    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn second_call_no_duplicate() {
    let dir = std::env::temp_dir().join("loopal_cr_nodup_v1");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("mem.md");
    fs::write(&path, "v1").unwrap();

    let snap = FileSnapshot::load(path.clone(), "Test");
    let mw = ConfigRefreshMiddleware::new(vec![snap]);

    wait_for_mtime();
    fs::write(&path, "v2").unwrap();

    let mut ctx1 = make_ctx(vec![Message::user("a")]);
    mw.process(&mut ctx1).await.unwrap();
    assert_eq!(ctx1.messages.len(), 2);

    let mut ctx2 = make_ctx(vec![Message::user("b")]);
    mw.process(&mut ctx2).await.unwrap();
    assert_eq!(ctx2.messages.len(), 1);

    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn system_prompt_unchanged() {
    let dir = std::env::temp_dir().join("loopal_cr_sysprompt_v1");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("mem.md");
    fs::write(&path, "old").unwrap();

    let snap = FileSnapshot::load(path.clone(), "Test");
    let mw = ConfigRefreshMiddleware::new(vec![snap]);

    wait_for_mtime();
    fs::write(&path, "new").unwrap();

    let mut ctx = make_ctx(vec![Message::user("hi")]);
    let original_prompt = ctx.system_prompt.clone();
    mw.process(&mut ctx).await.unwrap();
    assert_eq!(ctx.system_prompt, original_prompt);

    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn multiple_files_single_reminder() {
    let dir = std::env::temp_dir().join("loopal_cr_multi_v1");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let p1 = dir.join("mem.md");
    let p2 = dir.join("instr.md");
    fs::write(&p1, "mem_old").unwrap();
    fs::write(&p2, "instr_old").unwrap();

    let snaps = vec![
        FileSnapshot::load(p1.clone(), "Memory"),
        FileSnapshot::load(p2.clone(), "Instructions"),
    ];
    let mw = ConfigRefreshMiddleware::new(snaps);

    wait_for_mtime();
    fs::write(&p1, "mem_new").unwrap();
    fs::write(&p2, "instr_new").unwrap();

    let mut ctx = make_ctx(vec![Message::user("hi")]);
    mw.process(&mut ctx).await.unwrap();
    assert_eq!(ctx.messages.len(), 2);
    let text = ctx.messages[1].text_content();
    assert!(text.contains("Memory"));
    assert!(text.contains("Instructions"));

    let _ = fs::remove_dir_all(&dir);
}
