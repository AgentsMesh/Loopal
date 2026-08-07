"""Playwright Electron E2E target."""

load("@aspect_bazel_lib//lib:copy_to_directory.bzl", "copy_to_directory")
load("@npm//:@playwright/test/package_json.bzl", playwright_bin = "bin")

def playwright_test(name, srcs, data = None, config = "playwright.config.ts", env = None, package_json = "//:package_json_bin", tags = None, tsconfig = "tsconfig.json"):
    merged_tags = list(tags or [])

    # Electron suites launch real Loopal/Hub process trees. Bazel's local tag
    # only controls execution locality; exclusive prevents sibling E2E targets
    # from consuming their startup and IPC deadline budgets.
    for required in ["e2e", "exclusive", "manual", "no-sandbox", "local"]:
        if required not in merged_tags:
            merged_tags.append(required)
    source_tree = name + "_test_sources"
    copy_to_directory(
        name = source_tree,
        srcs = srcs + [config, package_json, tsconfig],
        hardlink = "off",
        tags = ["manual"],
    )
    playwright_bin.playwright_test(
        name = name,
        args = ["test", "--config", config, "--reporter=line"],
        data = [":" + source_tree, "//:node_modules"] + (data or []),
        env = dict(
            env or {},
            BAZEL_BINDIR = "$(BINDIR)",
            CI = "1",
            JS_BINARY__USE_EXECROOT_ENTRY_POINT = "1",
        ),
        chdir = native.package_name() + "/" + source_tree,
        size = "enormous",
        timeout = "eternal",
        tags = merged_tags,
    )
