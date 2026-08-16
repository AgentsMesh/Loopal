"""Desktop Rust integration-test target."""

load("@rules_rust//rust:defs.bzl", "rust_test")

def desktop_serve_test():
    rust_test(
        name = "desktop_serve_e2e_test",
        srcs = [
            "tests/e2e/desktop/serve.rs",
            "tests/e2e/desktop/session_phase.rs",
            "tests/e2e/desktop/startup_ui.rs",
            "tests/e2e/desktop/support.rs",
        ],
        crate_root = "tests/e2e/desktop/serve.rs",
        edition = "2024",
        data = [":loopal"],
        env = {"LOOPAL_BINARY": "$(rlocationpath :loopal)"},
        local = True,
        # The tests launch real Desktop/Hub processes and enforce startup
        # deadlines, so their timing must not include unrelated //... load.
        tags = ["exclusive"],
        deps = [
            "//crates/loopal-agent-client",
            "//crates/loopal-ipc",
            "@crates//:serde_json",
            "@crates//:tempfile",
            "@crates//:tokio",
        ],
    )
