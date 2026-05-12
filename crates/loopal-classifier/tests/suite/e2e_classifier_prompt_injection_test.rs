// E2E: classifier.md on disk → ConfigResolver → ClassifierEngine::with_question_system_prompt
// → ClassifierEngine::question_system_prompt() returns the custom content.
//
// This locks the cross-crate contract that user-customised prompts actually
// reach the classifier's runtime, not just sit in ResolvedConfig.

use std::fs;
use tempfile::tempdir;

use loopal_classifier::ClassifierEngine;
use loopal_config::layer::{ConfigLayer, LayerSource};
use loopal_config::loader::load_layer_from_dir;
use loopal_config::resolver::ConfigResolver;

#[test]
fn classifier_md_content_reaches_classifier_runtime() {
    // 1. Write a custom prompt to a project-layer dir
    let dir = tempdir().unwrap();
    let loopal_dir = dir.path().join(".loopal");
    fs::create_dir_all(&loopal_dir).unwrap();
    let custom_prompt = "PROJECT-RULE: always pick option B.";
    fs::write(loopal_dir.join("classifier.md"), custom_prompt).unwrap();

    // 2. Resolve via loader + resolver (mimics real config pipeline)
    let layer =
        load_layer_from_dir(&loopal_dir, LayerSource::Project, None).expect("layer load OK");
    let mut r = ConfigResolver::new();
    r.add_layer(layer);
    let resolved = r.resolve().expect("resolve OK");

    // 3. Construct ClassifierEngine the same way the production factory does
    let classifier =
        ClassifierEngine::new("".into()).with_question_system_prompt(resolved.classifier_prompt);

    // 4. The runtime accessor must return the user prompt, not the built-in default
    assert_eq!(classifier.question_system_prompt(), custom_prompt);
}

#[test]
fn no_classifier_md_falls_back_to_default_prompt() {
    let dir = tempdir().unwrap();
    let loopal_dir = dir.path().join(".loopal");
    fs::create_dir_all(&loopal_dir).unwrap();
    // Do NOT write classifier.md
    let layer = load_layer_from_dir(&loopal_dir, LayerSource::Project, None).unwrap();
    let mut r = ConfigResolver::new();
    r.add_layer(layer);
    let resolved = r.resolve().unwrap();
    assert!(resolved.classifier_prompt.is_none());

    let classifier =
        ClassifierEngine::new("".into()).with_question_system_prompt(resolved.classifier_prompt);
    let prompt = classifier.question_system_prompt();
    // Built-in default contains an unambiguous marker phrase
    assert!(
        prompt.contains("decision-making assistant"),
        "default prompt should be used, got: {}",
        &prompt[..120.min(prompt.len())]
    );
}

#[test]
fn project_layer_overrides_global_for_classifier_prompt() {
    let mut r = ConfigResolver::new();
    r.add_layer(ConfigLayer {
        source: LayerSource::Global,
        classifier_prompt: Some("GLOBAL".into()),
        ..Default::default()
    });
    r.add_layer(ConfigLayer {
        source: LayerSource::Project,
        classifier_prompt: Some("PROJECT".into()),
        ..Default::default()
    });
    let resolved = r.resolve().unwrap();

    let classifier =
        ClassifierEngine::new("".into()).with_question_system_prompt(resolved.classifier_prompt);
    assert_eq!(classifier.question_system_prompt(), "PROJECT");
}
