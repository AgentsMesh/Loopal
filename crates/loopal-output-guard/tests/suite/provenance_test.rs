use loopal_output_guard::{
    BinaryContentKind, BinaryProvenance, BinaryProvenanceError, require_known_binary_provenance,
};

#[test]
fn known_binary_provenance_releases_owned_bytes() {
    let value = require_known_binary_provenance(
        BinaryContentKind::Binary,
        BinaryProvenance::Known(b"mcp-response".to_vec()),
    )
    .unwrap();
    assert_eq!(value, b"mcp-response");
}

#[test]
fn unknown_binary_and_image_provenance_are_denied() {
    for kind in [BinaryContentKind::Binary, BinaryContentKind::Image] {
        let result = require_known_binary_provenance(kind, BinaryProvenance::Unknown);
        assert_eq!(result, Err(BinaryProvenanceError { kind }));
        assert!(format!("{}", result.unwrap_err()).contains("denied"));
    }
}
