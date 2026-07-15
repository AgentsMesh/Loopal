# Desktop runtime and security boundaries

This document defines the process, authority, IPC, recovery, and security contracts
between Electron and Loopal. The component overview is in
[Desktop architecture](./architecture.md); build mechanics are separate in
[build and release](./build-and-release.md).

## Process topology

```text
Sandboxed Renderer
  Workbench projection and user intent
        |
        | contextBridge + typed MessagePort, protocol v2
        v
Electron Main
  DesktopBackend
  SessionRuntimeRegistry (bounded live Hosts)
  expiring directory capabilities
  managed application MetaHub
        |
        +-- loopal desktop inspect-directory / prepare-worktree / cleanup-worktree
        |     one-shot, shell-free working-directory operation
        |
        `-- loopal desktop serve --parent-pid P [--resume SESSION]
              one Session Runtime generation
```

Rust is a sidecar, not a native Electron module. Main supervises each process and owns
restart, shutdown, protocol negotiation, and binary selection. The Renderer never sees
the binary path, process handle, discovery token, or raw Hub transport.

Each live Host may join a local or remote MetaHub. The managed coordinator can run with
zero joined Runtimes. Stopping a Runtime never stops the application coordinator;
`startLocalOnLaunch` and Runtime `joinOnStart` are independent policies.

## Host startup contract

Main starts a Session Host through a monotonic sequence:

```text
spawn with shell=false
  -> LOOPAL_DESKTOP alive + capabilities
  -> connect to the advertised loopback transport
  -> register the private UI client
  -> LOOPAL_DESKTOP_EVENT session_created (fresh Session only)
  -> commit directory capability and recovery metadata
  -> LOOPAL_DESKTOP ready + Session identity
  -> desktop/listSessions + view snapshot + redacted settings
  -> scoped Agent, attention, workspace, and MetaHub events
```

Restart waits for the old child to exit before allocating a newer generation. The
registry deduplicates concurrent resume, limits live and retired state, and drains
children during application shutdown. Parent-liveness monitoring makes an orphaned
sidecar terminate even when Electron cannot send a final request.

## IPC and control authority

Renderer and Main communicate through `ChannelClient` and `ChannelServer`. Main grants a
`MessageChannelMain` port only to the expected main frame. Zod validates command inputs,
results, events, and protocol version; cancellation is request-scoped.

Native image and Session-directory pickers use narrowly named Electron invoke channels
because the dialogs are Main-owned OS capabilities. They remain schema-validated facade
methods; preload exposes no generic invoke channel.

Preload exposes the `loopalDesktop` facade and no generic `invoke`, Node primitive, or
transport object. Main converts facade calls into explicit backend operations. The Rust
Hub applies its own UI-client ACL, so a compromised Main request still cannot discover
an undeclared raw method.

Data requests use a bounded lane. Shutdown, interrupt, permission, question, and plan
responses use control paths that cannot be starved by bulk projection traffic. Every
actionable event retains `workspaceId`, `sessionId`, `runtimeId`, and `generation`; Main
rejects stale responses and ambiguous Agent targets.

## Session and projection lifecycle

Loopal's Session store is authoritative. Bootstrap starts or resumes a cwd-scoped Host,
loads its Session catalog, merges stopped history, and attaches the live Runtime. Opening
a stopped Session is read-only until an explicit restart resumes it in its persisted
working directory.

Renderer state is replaceable projection state:

- Bootstrap buffers events until the authoritative snapshot is applied.
- Buffer loss or watcher overflow produces an explicit resync.
- Resume projects bounded persisted turns instead of replaying unbounded history.
- Agent refresh uses single-flight trailing invalidation.
- Retiring a Runtime clears its attention and generation-owned projections.
- Only one live Host forwards workspace invalidations for a workspace.

No retired generation may update conversation, topology, status, tasks, or attention.

## Working-directory authority

Session creation begins with a Main-owned native directory picker. Main canonicalizes
the selected path and invokes the one-shot Rust inspection command. Renderer receives a
bounded, expiring authorization ID rather than an arbitrary root path.

`createSession` consumes the capability once and revalidates the canonical selection.
For an isolated Git Worktree, inspection pins the full `HEAD` OID before creation under
`.loopal/worktrees/<name>` on branch `loopal-wt-<name>`.

Rollback is conservative. A fully drained pre-`alive` failure may clean a new Worktree;
an incomplete drain, unconfirmed exit, accepted `alive`, or emitted `session_created`
retains the directory and records recovery metadata. This avoids deleting a working
directory after process state has become commit-unknown.

The Rust workspace capability rejects absolute paths, traversal, and symlink escape;
bounds file/search/diff results; and uses expected-version atomic writes. These Host
operations remain protocol-tested even though Desktop exposes no Explorer, workspace
search, SCM, editor, standalone Worktree manager, terminal, xterm, or raw PTY surface.

Interactive execution belongs to Loopal Agent tools and their sandbox/permission flow,
not to a Renderer-controlled shell.

## Configuration ownership

Loopal atomically owns user defaults and provider credentials in
`~/.loopal/settings.json`. Project settings, MCP definitions, Skills, Plugins, and local
overrides stay in the selected directory's `.loopal/` hierarchy. The resolved settings
view explains plugin, user, project, local, and environment precedence without writing
the merged result back to a project file.

Electron owns Desktop presentation preferences, authorized Session recovery locations,
and application MetaHub settings/secrets needed before a Session Host exists. Secrets
are never returned to Renderer after being stored.

## Window and preload security

The main window uses context isolation, sandboxing, disabled Node integration, denied
permission requests/popups, HTTPS-only external opens, and blocked navigation. Packaged
launch ignores development URLs, fake backend selection, binary overrides, cwd
overrides, and hidden-E2E switches.

Future preview, browser, or artifact web content requires an isolated partition and a
minimal dedicated preload. It must not inherit the Workbench capability set.

## Boundary change checklist

1. Add or change a schema before implementing a new cross-process operation.
2. State the owner, scope, generation identity, cancellation, bounds, and redaction.
3. Prove stale-generation and shutdown behavior in units.
4. Exercise the actual process boundary in the appropriate
   [E2E layer](./e2e-contract.md).
5. Never solve a missing product workflow by exposing a raw Node, filesystem, Hub, or
   shell primitive.
