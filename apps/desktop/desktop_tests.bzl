"""Desktop Playwright suites."""

load("//build_defs/web:playwright.bzl", "playwright_test")

_ELECTRON_DATA = [
    ":out",
    "//:node_modules/@playwright/test",
    "//:node_modules/electron",
]

_HOST_DATA = _ELECTRON_DATA + [
    ":e2e_fixtures",
    "//:loopal",
    "//crates/loopal-mock-llm:loopal-mock-llm",
]

_MOCK_ENV = {
    "LOOPAL_MOCK_LLM_BINARY": "$(rootpath //crates/loopal-mock-llm:loopal-mock-llm)",
}

_COMMON_SUPPORT = [
    ":e2e_support_electron",
    ":e2e_support_fixtures",
    ":e2e_support_providers",
    ":e2e_support_runtime",
    ":e2e_support_settings",
]

_REAL_SUPPORT = _COMMON_SUPPORT + [":e2e_support_federation"]

def _declare_support_filegroups():
    for group in [
        "electron",
        "federation",
        "fixtures",
        "providers",
        "runtime",
        "settings",
    ]:
        native.filegroup(
            name = "e2e_support_" + group,
            srcs = native.glob(["e2e/support/%s/**/*.ts" % group]),
            tags = ["manual"],
        )

def desktop_playwright_tests():
    _declare_support_filegroups()
    playwright_test(
        name = "e2e",
        srcs = _COMMON_SUPPORT + native.glob(["e2e/fake/**/*.spec.ts"]),
        data = _ELECTRON_DATA,
    )

    playwright_test(
        name = "e2e_host",
        srcs = _REAL_SUPPORT + native.glob(["e2e/real/host/**/*.spec.ts"]),
        data = _HOST_DATA,
        env = _MOCK_ENV,
    )

    playwright_test(
        name = "e2e_llm_backend",
        srcs = _REAL_SUPPORT + native.glob(["e2e/real/provider/**/*.spec.ts"]),
        data = _HOST_DATA,
        env = _MOCK_ENV,
    )
