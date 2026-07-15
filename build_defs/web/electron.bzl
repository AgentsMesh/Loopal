"""Bazel-owned Electron build and packaging actions."""

load("@aspect_bazel_lib//lib:copy_file.bzl", "copy_file")
load("@aspect_bazel_lib//lib:copy_to_directory.bzl", "copy_to_directory")
load("@aspect_rules_js//js:defs.bzl", "js_run_binary")
load("@npm//:electron/package_json.bzl", electron_bin = "bin")
load("@npm//:electron-builder/package_json.bzl", electron_builder_bin = "bin")
load("@npm//:electron-vite/package_json.bzl", electron_vite_bin = "bin")
load(":desktop_package.bzl", "desktop_package_json")

def electron_vite_build(name, srcs, out_dir = "out", visibility = None, **kwargs):
    electron_vite_bin.electron_vite_binary(
        name = name + "_bin",
        visibility = ["//visibility:private"],
    )
    js_run_binary(
        name = name,
        srcs = srcs + ["//:node_modules"],
        args = ["build"],
        chdir = native.package_name(),
        out_dirs = [out_dir],
        tool = ":" + name + "_bin",
        visibility = visibility,
        **kwargs
    )

def electron_builder_app(
        name,
        out,
        sidecar,
        config = "electron-builder.yml",
        packaging_srcs = None,
        out_dir = "dist",
        platform = None,
        arch = None,
        visibility = None,
        tags = None):
    electron_builder_bin.electron_builder_binary(
        name = name + "_bin",
        patch_node_fs = False,
        visibility = ["//visibility:private"],
    )
    desktop_package_json(
        name = name + "_package_json",
        main = "./out/main/index.cjs",
    )
    copy_file(
        name = name + "_sidecar",
        src = sidecar,
        out = "runtime/loopal",
        allow_symlink = False,
    )
    copy_to_directory(
        name = name + "_staging",
        srcs = [
            out,
            ":" + name + "_package_json",
            ":" + name + "_sidecar",
            config,
        ] + (packaging_srcs or []),
        root_paths = [native.package_name()],
        hardlink = "off",
        allow_overwrites = True,
        verbose = False,
    )
    args = ["--config", config, "--projectDir", name + "_staging"]
    if platform:
        args.append("--" + platform)
    if arch:
        args.append("--" + arch)
    js_run_binary(
        name = name,
        srcs = [":" + name + "_staging", "//:node_modules"],
        args = args,
        chdir = native.package_name(),
        out_dirs = [out_dir],
        tool = ":" + name + "_bin",
        tags = tags or ["manual", "electron_builder", "no-sandbox", "local"],
        visibility = visibility,
        patch_node_fs = False,
        env = {
            "CSC_IDENTITY_AUTO_DISCOVERY": "false",
        },
    )

def electron_app(name, out, sidecar = None, fake_backend = False):
    data = [out, "//:node_modules/electron"]
    env = {}
    if sidecar:
        data.append(sidecar)
        env["LOOPAL_DESKTOP_BINARY_RUNFILE"] = "$(rlocationpath %s)" % sidecar
    if fake_backend:
        env["LOOPAL_DESKTOP_BACKEND"] = "fake"
    electron_bin.electron_binary(
        name = name,
        args = ["out/main/index.cjs"],
        data = data,
        env = env,
        chdir = native.package_name(),
        tags = ["manual", "electron", "local"],
    )
