"""Root cross-compilation platform targets."""

def loopal_platforms():
    native.platform(
        name = "linux-x86_64",
        constraint_values = [
            "@platforms//os:linux",
            "@platforms//cpu:x86_64",
        ],
    )
    native.platform(
        name = "linux-aarch64",
        constraint_values = [
            "@platforms//os:linux",
            "@platforms//cpu:aarch64",
        ],
    )
    native.platform(
        name = "macos-x86_64",
        constraint_values = [
            "@platforms//os:macos",
            "@platforms//cpu:x86_64",
        ],
    )
    native.platform(
        name = "macos-aarch64",
        constraint_values = [
            "@platforms//os:macos",
            "@platforms//cpu:aarch64",
        ],
    )
    native.platform(
        name = "windows-x86_64",
        constraint_values = [
            "@platforms//os:windows",
            "@platforms//cpu:x86_64",
        ],
    )
