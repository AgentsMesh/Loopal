"""Root npm targets consumed by Bazel-owned desktop actions."""

load("@aspect_bazel_lib//lib:copy_to_bin.bzl", "copy_to_bin")
load("@npm//:defs.bzl", "npm_link_all_packages")

def desktop_npm_workspace():
    npm_link_all_packages(name = "node_modules")
    copy_to_bin(
        name = "package_json_bin",
        srcs = ["package.json"],
        visibility = ["//visibility:public"],
    )
    native.exports_files(
        ["package.json", "pnpm-lock.yaml", "pnpm-workspace.yaml"],
        visibility = ["//visibility:public"],
    )
