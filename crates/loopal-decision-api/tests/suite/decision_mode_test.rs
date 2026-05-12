use loopal_decision_api::DecisionMode;

#[test]
fn default_is_manual() {
    assert_eq!(DecisionMode::default(), DecisionMode::Manual);
}

#[test]
fn serde_serializes_as_snake_case() {
    assert_eq!(
        serde_json::to_string(&DecisionMode::Manual).unwrap(),
        "\"manual\""
    );
    assert_eq!(
        serde_json::to_string(&DecisionMode::Classifier).unwrap(),
        "\"classifier\""
    );
    assert_eq!(
        serde_json::to_string(&DecisionMode::Agent).unwrap(),
        "\"agent\""
    );
}

#[test]
fn serde_deserializes_canonical_names() {
    let manual: DecisionMode = serde_json::from_str("\"manual\"").unwrap();
    assert_eq!(manual, DecisionMode::Manual);
    let classifier: DecisionMode = serde_json::from_str("\"classifier\"").unwrap();
    assert_eq!(classifier, DecisionMode::Classifier);
    let agent: DecisionMode = serde_json::from_str("\"agent\"").unwrap();
    assert_eq!(agent, DecisionMode::Agent);
}

#[test]
fn serde_rejects_legacy_auto_string() {
    // `auto` was the pre-three-mode variant name; we no longer accept it.
    let v = serde_json::from_str::<DecisionMode>("\"auto\"");
    assert!(v.is_err(), "auto must NOT deserialize as a valid mode");
}

#[test]
fn serde_rejects_unknown_variant() {
    let err = serde_json::from_str::<DecisionMode>("\"not_a_mode\"");
    assert!(err.is_err());
}

#[test]
fn copy_and_eq_semantics() {
    let mode = DecisionMode::Classifier;
    let copy = mode;
    assert_eq!(mode, copy);
}

#[test]
fn from_str_accepts_known_variants() {
    assert_eq!(
        "manual".parse::<DecisionMode>().unwrap(),
        DecisionMode::Manual
    );
    assert_eq!(
        "classifier".parse::<DecisionMode>().unwrap(),
        DecisionMode::Classifier
    );
    assert_eq!(
        "agent".parse::<DecisionMode>().unwrap(),
        DecisionMode::Agent
    );
}

#[test]
fn from_str_rejects_legacy_auto() {
    assert!("auto".parse::<DecisionMode>().is_err());
}

#[test]
fn from_str_rejects_unknown_with_descriptive_error() {
    let err = "not_a_mode".parse::<DecisionMode>().unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("not_a_mode"),
        "error must mention input: {msg}"
    );
    assert!(
        msg.contains("manual"),
        "error must mention valid variant: {msg}"
    );
}

#[test]
fn display_round_trips_with_from_str() {
    for mode in [
        DecisionMode::Manual,
        DecisionMode::Classifier,
        DecisionMode::Agent,
    ] {
        let s = mode.to_string();
        assert_eq!(s.parse::<DecisionMode>().unwrap(), mode);
    }
}
