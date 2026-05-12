use std::fs;
use tempfile::tempdir;

use loopal_config::layer::{ConfigLayer, LayerSource};
use loopal_config::loader::load_layer_from_dir;
use loopal_config::resolver::ConfigResolver;

#[test]
fn classifier_md_loaded_from_dir() {
    let dir = tempdir().unwrap();
    let loopal_dir = dir.path().join(".loopal");
    fs::create_dir_all(&loopal_dir).unwrap();
    let prompt = "## My Custom Classifier\nAlways pick option A.\n";
    fs::write(loopal_dir.join("classifier.md"), prompt).unwrap();

    let layer = load_layer_from_dir(&loopal_dir, LayerSource::Project, None).unwrap();
    assert_eq!(layer.classifier_prompt.as_deref(), Some(prompt));
}

#[test]
fn classifier_md_absent_yields_none() {
    let dir = tempdir().unwrap();
    let loopal_dir = dir.path().join(".loopal");
    fs::create_dir_all(&loopal_dir).unwrap();

    let layer = load_layer_from_dir(&loopal_dir, LayerSource::Project, None).unwrap();
    assert!(layer.classifier_prompt.is_none());
}

#[test]
fn higher_priority_layer_overrides_lower_classifier_prompt() {
    let mut r = ConfigResolver::new();
    r.add_layer(ConfigLayer {
        source: LayerSource::Global,
        classifier_prompt: Some("global prompt".into()),
        ..Default::default()
    });
    r.add_layer(ConfigLayer {
        source: LayerSource::Project,
        classifier_prompt: Some("project prompt".into()),
        ..Default::default()
    });
    let resolved = r.resolve().unwrap();
    assert_eq!(
        resolved.classifier_prompt.as_deref(),
        Some("project prompt")
    );
}

#[test]
fn empty_classifier_prompt_does_not_override_lower_layer() {
    let mut r = ConfigResolver::new();
    r.add_layer(ConfigLayer {
        source: LayerSource::Global,
        classifier_prompt: Some("real prompt".into()),
        ..Default::default()
    });
    r.add_layer(ConfigLayer {
        source: LayerSource::Project,
        classifier_prompt: Some("   ".into()),
        ..Default::default()
    });
    let resolved = r.resolve().unwrap();
    assert_eq!(resolved.classifier_prompt.as_deref(), Some("real prompt"));
}

#[test]
fn no_layer_with_classifier_prompt_yields_none() {
    let mut r = ConfigResolver::new();
    r.add_layer(ConfigLayer {
        source: LayerSource::Global,
        ..Default::default()
    });
    r.add_layer(ConfigLayer {
        source: LayerSource::Project,
        ..Default::default()
    });
    let resolved = r.resolve().unwrap();
    assert!(resolved.classifier_prompt.is_none());
}
