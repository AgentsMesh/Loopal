"""Rust tests owned by the root Bazel package."""

load("@rules_rust//rust:defs.bzl", "rust_binary", "rust_test")
load("//build_defs/rust:desktop_test.bzl", "desktop_serve_test")

def _binary_e2e(name, src, deps):
    rust_test(
        name = name,
        srcs = [src],
        data = [":loopal"],
        edition = "2024",
        env = {"LOOPAL_BINARY": "$(rootpath :loopal)"},
        local = True,
        deps = deps,
    )

def loopal_root_tests():
    _binary_e2e(
        name = "system_ipc_test",
        src = "tests/e2e/system_ipc.rs",
        deps = [
            "//crates/loopal-ipc",
            "//crates/loopal-protocol",
            "@crates//:serde_json",
            "@crates//:tempfile",
            "@crates//:tokio",
        ],
    )
    _binary_e2e(
        name = "hub_lifecycle_test",
        src = "tests/e2e/hub_lifecycle.rs",
        deps = [
            "@crates//:anyhow",
            "@crates//:serde_json",
            "@crates//:tempfile",
            "@crates//:tokio",
        ],
    )
    _binary_e2e(
        name = "join_hub_e2e_test",
        src = "tests/e2e/join_hub.rs",
        deps = [
            "@crates//:anyhow",
            "@crates//:serde_json",
            "@crates//:tempfile",
            "@crates//:tokio",
        ],
    )
    _binary_e2e(
        name = "hub_only_mcp_deadlock_test",
        src = "tests/regressions/hub_only_mcp_deadlock.rs",
        deps = [
            "@crates//:serde_json",
            "@crates//:tempfile",
            "@crates//:tokio",
        ],
    )
    _binary_e2e(
        name = "bootstrap_typestate_e2e_test",
        src = "tests/e2e/bootstrap_typestate.rs",
        deps = [
            "@crates//:tempfile",
            "@crates//:tokio",
        ],
    )
    rust_test(
        name = "cli_llm_e2e_test",
        srcs = native.glob(["tests/e2e/cli_llm/*.rs"]),
        crate_root = "tests/e2e/cli_llm/suite.rs",
        data = [":loopal"],
        edition = "2024",
        env = {"LOOPAL_BINARY": "$(rootpath :loopal)"},
        local = True,
        deps = [
            "//crates/loopal-ipc",
            "//crates/loopal-mock-llm:loopal-mock-llm-lib",
            "//crates/loopal-protocol",
            "@crates//:chrono",
            "@crates//:reqwest",
            "@crates//:serde_json",
            "@crates//:tempfile",
            "@crates//:tokio",
            "@crates//:uuid",
        ],
    )
    rust_binary(
        name = "mock_mcp_server",
        srcs = ["tests/fixtures/mock_mcp_server/main.rs"],
        edition = "2024",
        deps = ["@crates//:serde_json"],
    )
    rust_test(
        name = "hub_llm_e2e_test",
        srcs = native.glob(["tests/e2e/hub_llm/*.rs"]),
        crate_root = "tests/e2e/hub_llm/suite.rs",
        data = [
            ":loopal",
            ":mock_mcp_server",
        ],
        edition = "2024",
        env = {
            "LOOPAL_BINARY": "$(rootpath :loopal)",
            "LOOPAL_MOCK_MCP_BINARY": "$(rootpath :mock_mcp_server)",
        },
        local = True,
        deps = [
            "//crates/loopal-ipc",
            "//crates/loopal-mock-llm:loopal-mock-llm-lib",
            "//crates/loopal-protocol",
            "@crates//:reqwest",
            "@crates//:serde_json",
            "@crates//:tempfile",
            "@crates//:tokio",
        ],
    )
    desktop_serve_test()
    rust_test(
        name = "loopal-unit-test",
        crate = ":loopal",
        edition = "2024",
    )
    rust_test(
        name = "architecture_boundary_test",
        srcs = ["tests/architecture/boundaries.rs"],
        edition = "2024",
        local = True,
        deps = [
            "@crates//:regex",
            "@crates//:walkdir",
        ],
    )
