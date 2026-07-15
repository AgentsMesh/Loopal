# Desktop build and release

Loopal Desktop is Bazel-only. pnpm resolves the checked-in JavaScript lockfile, while
Bazel owns TypeScript checking, Electron compilation, unit tests, coverage, E2E,
staging, packaging, and application launch. Do not add npm/pnpm build or test scripts.

## Build inputs

| Input | Responsibility |
| --- | --- |
| `MODULE.bazel` / lock | pinned Node, TypeScript, rules_js/rules_ts, and Rust toolchains |
| `package.json` / `pnpm-lock.yaml` | JavaScript dependency resolution |
| `build_defs/web` | Electron, Vitest, Playwright, npm workspace, and package rules |
| `apps/desktop/BUILD.bazel` | Desktop source, typecheck, unit, coverage, app, and package targets |
| `apps/desktop/desktop_tests.bzl` | physical E2E suite-to-target mapping |
| `apps/desktop/electron-builder.yml` | packaged application metadata and resources |

There are currently no nested Bazel packages under `apps/desktop/src`. Its root
`glob()` owns all source files. Adding a nested `BUILD` file would stop the parent glob
at that directory, so child targets and parent aggregation must be introduced atomically.

## Target graph

```text
pnpm-lock.yaml -> //:node_modules
Desktop sources -> //apps/desktop:typecheck -> //apps/desktop:out
unit sources -> //apps/desktop:unit -> //apps/desktop:coverage
fake Electron -> //apps/desktop:e2e
real Electron + //:loopal -> //apps/desktop:e2e_host
provider scenarios + Mock LLM + //:loopal -> //apps/desktop:e2e_llm_backend
:out + stamped metadata + //:loopal -> :dist_staging -> :dist
```

Primary build and launch commands:

```bash
bazel build //apps/desktop:typecheck
bazel build //apps/desktop:out
bazel run //apps/desktop:app
bazel run //apps/desktop:app_fake
bazel build //apps/desktop:dist -c opt
```

Verification commands and suite ownership are defined in
[testing](./testing.md) and [the E2E contract](./e2e-contract.md).

## Electron output

`//apps/desktop:out` runs the pinned Electron/Vite build and emits Main, preload, and
Renderer output. Main and preload entry points use `.cjs`; Renderer assets remain
sandbox-compatible static resources. Source maps and development-only behavior are not
packaged authority.

`//apps/desktop:app` launches the output with the Bazel-built `//:loopal` sidecar.
`//apps/desktop:app_fake` is a development/test surface and never enables fake mode in a
packaged application.

## Packaging pipeline

One packaging action emits the current host platform/architecture:

```text
//apps/desktop:out
  -> writable electron-builder staging copy
  -> stamped package metadata
  -> Bazel-built Loopal sidecar in Resources/bin
  -> //apps/desktop:dist_staging
  -> //apps/desktop:dist
```

The matching sidecar is copied to `Resources/bin/loopal` (or `loopal.exe`) outside
`app.asar` and remains executable. macOS includes it in the hardened-runtime binary
list. electron-builder consumes Bazel's pinned unpacked Electron distribution; it must
not run a package manager or download Electron.

The writable output copy is prepared after Bazel extraction so immutable inputs are
never modified. Implicit macOS keychain identity discovery is disabled. Signed releases
must receive explicit CI credentials and entitlements.

## Staging verification

```bash
bazel test //apps/desktop:staging_smoke -c opt --test_output=errors
```

The smoke test inspects the unpackaged staging tree. It verifies:

- the Main and preload `.cjs` entries exist;
- stamped metadata points to the correct Main entry;
- the bundled sidecar exists outside `app.asar` and is executable;
- the staged sidecar can execute `--version`.

It does not launch or install the application. Runtime behavior belongs to Electron E2E.

## Release checklist

1. Build and test through Bazel; do not invoke Vite, Vitest, Playwright, or
   electron-builder directly.
2. Run the relevant Desktop unit and E2E targets from [testing](./testing.md).
3. Run staging smoke and inspect the platform-specific artifact.
4. Confirm no development/fake/binary override can activate in packaged mode.
5. Supply signing credentials only in the release environment.
6. Keep generated output and coverage outside source-controlled directories.
