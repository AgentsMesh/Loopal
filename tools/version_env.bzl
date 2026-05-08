"""Extract STABLE_LOOPAL_VERSION from Bazel's stable-status.txt into a rustc env file."""

def _version_env_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".env")
    ctx.actions.run_shell(
        outputs = [out],
        inputs = [ctx.info_file],
        command = """
            if grep -q '^STABLE_LOOPAL_VERSION ' "{src}"; then
                v=$(grep '^STABLE_LOOPAL_VERSION ' "{src}" | head -1 | cut -d' ' -f2-)
                printf 'LOOPAL_VERSION=%s\n' "$v" > "{out}"
            else
                printf 'LOOPAL_VERSION=0.0.0-unstamped\n' > "{out}"
            fi
        """.format(src = ctx.info_file.path, out = out.path),
        progress_message = "Generating LOOPAL_VERSION env",
        mnemonic = "LoopalVersionEnv",
    )
    return [DefaultInfo(files = depset([out]))]

version_env = rule(
    implementation = _version_env_impl,
)
