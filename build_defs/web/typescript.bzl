"""Hermetic TypeScript checks backed by the workspace npm graph."""

load("@aspect_rules_js//js:defs.bzl", "js_run_binary")
load("@npm//:typescript/package_json.bzl", typescript_bin = "bin")

def typescript_check(name, srcs, tsconfig = "tsconfig.json", data = None, visibility = None):
    tool = name + "_bin"
    typescript_bin.tsc_binary(
        name = tool,
        visibility = ["//visibility:private"],
    )
    js_run_binary(
        name = name,
        tool = ":" + tool,
        srcs = srcs + [tsconfig] + (data or []),
        args = [
            "--project",
            tsconfig,
            "--noEmit",
            "--pretty",
            "false",
        ],
        chdir = native.package_name(),
        stdout = name + ".log",
        mnemonic = "TypeScriptCheck",
        visibility = visibility,
    )
