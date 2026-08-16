use loopal_protocol::PermissionReceiptError;

#[test]
fn receipt_errors_have_stable_display_messages() {
    let messages = [
        (
            PermissionReceiptError::Binding,
            "permission receipt binding mismatch",
        ),
        (
            PermissionReceiptError::Generation,
            "permission receipt generation is invalid",
        ),
        (
            PermissionReceiptError::Token,
            "permission receipt token is invalid",
        ),
        (
            PermissionReceiptError::Consumed,
            "permission receipt was already consumed",
        ),
        (
            PermissionReceiptError::Registry,
            "permission receipt registry unavailable",
        ),
    ];
    for (error, expected) in messages {
        assert_eq!(error.to_string(), expected);
    }
}
