# Loopal Desktop architecture

Loopal Desktop is an Agent workbench whose bundled kernel is the `loopal` CLI.
Electron owns application presentation and OS integration. Loopal owns Sessions,
Agents, policy, tools, durable state, and governed workspace execution.

This document owns the component model and architectural direction. Detailed contracts
live in:

- [Runtime boundaries](./runtime-boundaries.md) for processes, IPC, authority, recovery,
  and security.
- [Experience model](./experience-model.md) for the user-facing state and interaction
  model.
- [Build and release](./build-and-release.md) for Bazel targets and packaging.
- [Repository layout](../repository-layout.md) for physical directories and dependency
  direction.

## Reference findings

AgentsMesh provides the Bazel-only Electron pattern: pinned Node inputs,
`electron-vite`, stamped metadata, electron-builder staging, a bundled sidecar, and
Playwright targets. Loopal remains a sidecar rather than an ABI-bound N-API addon so
crashes, restart, and future remote placement share one process boundary.

Synapse contributes `base`, `platform`, and `workbench` layering; lifecycle, event, and
cancellation primitives; scoped channels; and pane/layout registries. Loopal Desktop
keeps explicit validated domain operations instead of generic reflection, arbitrary
Renderer invoke, or parallel IPC stacks.

## Architectural invariants

- Bazel is the only build, test, E2E, staging, and packaging entry.
- Rust is a supervised child process, never an Electron native addon.
- The Renderer has no Node API, token, binary path, or raw Hub method.
- Main exposes one typed Desktop API with schema-validated commands and events.
- Session, Runtime, Agent, conversation, task, and artifact remain distinct concepts.
- One Session has at most one live Runtime; retired generations cannot affect a newer one.
- MetaHub coordination is application-scoped; membership is Runtime-scoped.
- Raw PTY and arbitrary filesystem access are not Desktop capabilities.
- App services outlive windows; window connections are disposed with their window.
- Handwritten production and test files should stay at or below 200 lines.

## Component model

```text
Renderer
  Workbench shell
    Conversation contributions
    Federation contribution
    dynamic Session panels
    Settings overlay
        |
        | typed Desktop API
        v
Electron Main
  DesktopBackend
  SessionRuntimeRegistry
  managed MetaHub coordinator
  directory authorization
        |
        | supervised loopback/stdio protocol
        v
Loopal sidecars
  Session Host -> Hub -> root and child Agents
  one-shot working-directory operations
```

Main is the application composition layer. Platform services implement transport,
Host lifecycle, Session catalog/projection, attention, settings, workspace authority,
and Federation coordination. Renderer features consume only the Desktop contract and
authoritative projections.

## Domain ownership

The user hierarchy is Session → Runtime → Agent.

- A Session is durable conversation identity and working-directory context.
- A Runtime is one live generation of a Session.
- An Agent is a routed execution participant within a Runtime/Hub topology.
- A Workspace ID is internal authorization/configuration scope; it does not group the
  Session navigator.
- Federation aggregates qualified Agent observations across live Runtime generations.

Loopal's Session store is authoritative for conversation history and Agent state.
Desktop persists only application preferences, authorization/recovery metadata, and
MetaHub application settings that Electron must restore. Scope and precedence details
belong to [runtime boundaries](./runtime-boundaries.md).

## Workbench composition

`workbench/browser` owns the stable shell: application controller, view state,
navigation chrome, Session workspace composition, status, localization context, and
global shortcuts. Feature UI, tests, and CSS live under:

```text
workbench/contrib/
  conversation/    transcript, composer, Markdown, tools, slash commands
  sessions/        catalog, creation, navigator, Runtime status
  attention/       permissions, questions, and plan approval
  agents/          Agent topology and controls
  session-panels/  dynamic artifacts, tasks, MCP, and diagnostics
  federation/      aggregate topology and remote conversations
  settings/        scoped Desktop, Loopal, MCP, Skills, and MetaHub settings
```

Conversation is the default resumable surface. Runtime-generated panels appear only
when projected content is meaningful. Federation is independent application navigation
and never inherits Session toolbar chrome. Settings overlays the active product area.

## Projection rules

Every live observation retains its exact `(sessionId, runtimeId, generation)` owner.
Remote identity additionally includes Hub origin. A stale or ambiguous projection may
remain inspectable, but controls resolve only to one current live owner.

Renderer state is a projection, not execution authority. Bootstrap applies an
authoritative snapshot and then replays buffered events; subsequent resync replaces
affected projection state rather than merging guesses.

## Evolution path

1. Conversation fidelity: ordered events, Agent drafts, panels, and attention.
2. Federation hardening: qualified identity, degraded state, and exact routing.
3. Closed loop: governed preview/browser capabilities and DOM/image checks.
4. General workbench: artifacts, routines, connectors, Skills, and Plugins.
5. Distributed work: SSH/cloud Runtimes, handoff, and remote routines.

Each phase must preserve the process, security, and generation boundaries rather than
adding a Renderer-side shortcut around them.
