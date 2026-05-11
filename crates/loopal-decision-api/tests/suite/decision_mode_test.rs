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
        serde_json::to_string(&DecisionMode::Auto).unwrap(),
        "\"auto\""
    );
}

#[test]
fn serde_deserializes_snake_case() {
    let manual: DecisionMode = serde_json::from_str("\"manual\"").unwrap();
    assert_eq!(manual, DecisionMode::Manual);
    let auto: DecisionMode = serde_json::from_str("\"auto\"").unwrap();
    assert_eq!(auto, DecisionMode::Auto);
}

#[test]
fn serde_rejects_unknown_variant() {
    let err = serde_json::from_str::<DecisionMode>("\"magic\"");
    assert!(err.is_err());
}

#[test]
fn copy_and_eq_semantics() {
    let mode = DecisionMode::Auto;
    let copy = mode;
    assert_eq!(mode, copy);
}

#[test]
fn from_str_accepts_known_variants() {
    assert_eq!("manual".parse::<DecisionMode>().unwrap(), DecisionMode::Manual);
    assert_eq!("auto".parse::<DecisionMode>().unwrap(), DecisionMode::Auto);
}

#[test]
fn from_str_rejects_unknown_with_descriptive_error() {
    let err = "magic".parse::<DecisionMode>().unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("magic"), "error must mention input: {msg}");
    assert!(msg.contains("manual"), "error must mention valid variant: {msg}");
}

#[test]
fn display_round_trips_with_from_str() {
    for mode in [DecisionMode::Manual, DecisionMode::Auto] {
        let s = mode.to_string();
        assert_eq!(s.parse::<DecisionMode>().unwrap(), mode);
    }
}
