# Loopal Desktop documentation

Loopal Desktop is a conversation-first Electron workbench with the Bazel-built `loopal`
CLI as its bundled runtime sidecar. This directory separates product experience,
runtime/security boundaries, verification, and release mechanics so each fact has one
authoritative home.

Start with the repository-wide [layout and dependency boundaries](../repository-layout.md).

## Document index

| Document | Owns |
| --- | --- |
| [Architecture](./architecture.md) | Desktop goals, component model, invariants, and evolution path |
| [Runtime boundaries](./runtime-boundaries.md) | Electron/Loopal process lifecycle, IPC, workspace authority, recovery, and security |
| [Experience model](./experience-model.md) | Conversation, Federation, navigation, settings, panels, focus, and responsive behavior |
| [Slash commands](./slash-commands.md) | Composer command routing and typed control/data-plane separation |
| [Testing](./testing.md) | Bazel verification targets, unit/coverage scope, and Rust gates |
| [E2E contract](./e2e-contract.md) | Fake, real Host, and provider-boundary acceptance suites and fixtures |
| [Build and release](./build-and-release.md) | Bazel Desktop graph, development launch, staging, packaging, and signing |

## Reading paths

- Product/UI work: Architecture → Experience model → relevant feature contract.
- Runtime/IPC work: Architecture → Runtime boundaries → E2E contract.
- Test work: Testing → E2E contract.
- Packaging/release work: Build and release → Testing.

The physical `apps/desktop` tree is documented only in
[repository layout](../repository-layout.md); do not duplicate a second directory map
here. Bazel labels and checked-in schemas remain the executable source of truth when a
document and code disagree.
