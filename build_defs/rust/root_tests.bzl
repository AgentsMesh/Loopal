"""Rust tests owned by the root Bazel package."""

load("@rules_rust//rust:defs.bzl", "rust_binary", "rust_library", "rust_test")
load("//build_defs/rust:desktop_test.bzl", "desktop_serve_test")

def _binary_e2e(name, src, deps, extra_srcs = [], tags = []):
    # These targets launch the real Loopal binary and assert wall-clock
    # handshakes. Keep process startup out of the shared //... resource pool.
    rust_test(
        name = name,
        srcs = [src] + extra_srcs,
        crate_root = src,
        data = [":loopal"],
        edition = "2024",
        env = {"LOOPAL_BINARY": "$(rlocationpath :loopal)"},
        local = True,
        tags = ["exclusive"] + tags,
        deps = ["//crates/loopal-agent-client"] + deps,
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
        extra_srcs = ["tests/e2e/startup_ui_gate.rs"],
        deps = [
            "//crates/loopal-ipc",
            "@crates//:serde_json",
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
        env = {"LOOPAL_BINARY": "$(rlocationpath :loopal)"},
        local = True,
        # reason: excluded from `//...` wildcards (three-OS CI matrix) and run
        # by the dedicated Agent E2E gate job, mirroring the desktop e2e setup.
        tags = [
            "e2e",
            "exclusive",
            "manual",
        ],
        deps = [
            "//crates/loopal-agent-client",
            "//crates/loopal-ipc",
            "//crates/loopal-protocol",
            "//crates/loopal-test-support:mock-llm-server",
            "@crates//:chrono",
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
    rust_binary(
        name = "mock_workflow_worker",
        srcs = ["tests/fixtures/mock_workflow_worker/main.rs"],
        edition = "2024",
        deps = [
            "//crates/loopal-ipc",
            "//crates/loopal-protocol",
            "@crates//:serde_json",
            "@crates//:tokio",
        ],
    )
    rust_test(
        name = "hub_llm_e2e_test",
        srcs = native.glob(["tests/e2e/hub_llm/*.rs"]),
        crate_root = "tests/e2e/hub_llm/suite.rs",
        data = [
            ":loopal",
            ":mock_mcp_server",
            ":mock_workflow_worker",
        ],
        edition = "2024",
        env = {
            "LOOPAL_BINARY": "$(rlocationpath :loopal)",
            "LOOPAL_MOCK_MCP_BINARY": "$(rlocationpath :mock_mcp_server)",
            "LOOPAL_MOCK_WORKFLOW_WORKER_BINARY": "$(rlocationpath :mock_workflow_worker)",
        },
        local = True,
        tags = [
            "e2e",
            "exclusive",
            "manual",
        ],
        deps = [
            "//crates/loopal-agent-client",
            "//crates/loopal-ipc",
            "//crates/loopal-protocol",
            "//crates/loopal-storage",
            "//crates/loopal-test-support:mock-llm-server",
            "//crates/loopal-turn",
            "//crates/loopal-vault-age",
            "//crates/loopal-vault-api",
            "@crates//:secrecy",
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
        deps = ["@crates//:tempfile"],
    )

    # A crate-backed test is required here so rules_rust propagates the real
    # Rust source and LLVM tools into split coverage post-processing.
    rust_library(
        name = "bootstrap_workflow_runtime_coverage",
        srcs = [
            "src/bootstrap/hub/typestate/workflow_runtime.rs",
            "src/bootstrap/hub/typestate/workflow_runtime_tests.rs",
            "tests/coverage/bootstrap_workflow_runtime.rs",
        ],
        crate_root = "tests/coverage/bootstrap_workflow_runtime.rs",
        edition = "2024",
        tags = ["manual"],
        testonly = True,
        deps = ["@crates//:tokio"],
    )
    rust_test(
        name = "bootstrap_workflow_runtime_coverage_test",
        crate = ":bootstrap_workflow_runtime_coverage",
        edition = "2024",
        tags = ["manual"],
    )
    rust_library(
        name = "bootstrap_start_root_coverage",
        srcs = [
            "src/bootstrap/hub/typestate/start_root.rs",
            "tests/coverage/bootstrap_start_root.rs",
            "tests/coverage/bootstrap_start_root_boundary.rs",
            "tests/coverage/bootstrap_start_root_tests.rs",
        ],
        crate_root = "tests/coverage/bootstrap_start_root.rs",
        edition = "2024",
        tags = ["manual"],
        testonly = True,
        deps = ["@crates//:tokio"],
    )
    rust_test(
        name = "bootstrap_start_root_coverage_test",
        crate = ":bootstrap_start_root_coverage",
        edition = "2024",
        tags = ["manual"],
    )
    rust_test(
        name = "bootstrap_lifecycle_test",
        crate = ":loopal",
        args = [
            "--include-ignored",
            "bootstrap::",
            "--test-threads=1",
        ],
        data = [
            ":loopal",
            "tests/fixtures/bootstrap_mock_provider.json",
        ],
        edition = "2024",
        env = {
            "LOOPAL_BINARY": "$(rlocationpath :loopal)",
            "LOOPAL_OTEL_ENABLED": "0",
            "LOOPAL_TEST_PROVIDER": "$(rlocationpath tests/fixtures/bootstrap_mock_provider.json)",
        },
        local = True,
        tags = ["exclusive"],
        deps = ["@crates//:tempfile"],
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
