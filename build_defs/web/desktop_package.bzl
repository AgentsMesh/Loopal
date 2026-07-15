"""Stamped Electron package metadata."""

def _desktop_package_json_impl(ctx):
    out = ctx.actions.declare_file("package.json")
    ctx.actions.run_shell(
        inputs = [ctx.info_file],
        outputs = [out],
        command = """
v=$(awk '/^STABLE_LOOPAL_VERSION / {{print substr($0, index($0,$2))}}' "{status}")
test -n "$v" || v=0.0.0-unstamped
printf '{{"name":"loopal-desktop","version":"%s","private":true,"description":"Loopal agent workbench","author":{{"name":"AgentsMesh","email":"support@agentsmesh.ai"}},"homepage":"https://agentsmesh.ai","main":"{main}"}}\n' "$v" > "{out}"
""".format(status = ctx.info_file.path, main = ctx.attr.main, out = out.path),
        mnemonic = "LoopalDesktopPackageJson",
    )
    return [DefaultInfo(files = depset([out]))]

desktop_package_json = rule(
    implementation = _desktop_package_json_impl,
    attrs = {"main": attr.string(mandatory = True)},
)
