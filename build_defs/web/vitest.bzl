"""Vitest targets with an enforceable coverage gate.

Vitest's package exports do not provide a reliable CommonJS CLI entry under
Bazel's pnpm link tree. A tiny checked-in shim resolves the package from the
test working directory, launches its ESM binary, and preserves Node's ESM
interop option in worker processes. This is the same Bazel-only boundary used
by AgentsMesh rather than a package-manager script fallback.
"""

load("@aspect_bazel_lib//lib:copy_to_directory.bzl", "copy_to_directory")
load("@aspect_rules_js//js:defs.bzl", "js_test")

def vitest_test(name, srcs, config = "vitest.config.ts", coverage = False, data = None, tags = None):
    args = ["run", "--config", config]
    if coverage:
        args.append("--coverage")
    source_tree = name + "_test_sources"
    copy_to_directory(
        name = source_tree,
        srcs = srcs + [config] + (data or []),
        allow_overwrites = True,
        hardlink = "off",
        tags = ["manual"],
    )
    js_test(
        name = name,
        entry_point = "//build_defs/web:vitest_shim",
        args = args + ["--reporter=default", "--no-color"],
        data = [
            ":" + source_tree,
            "//:node_modules",
            "//:node_modules/vitest",
        ],
        env = {
            "VITEST": "true",
            "VITEST_RUNFILES_WORKDIR": "$(rlocationpath :%s)" % source_tree,
        },
        size = "large",
        tags = tags or [],
    )
