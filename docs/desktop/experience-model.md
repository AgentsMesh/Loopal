# Loopal Desktop experience model

Loopal Desktop is a conversation-first Agent workbench. Conversation commissions and
explains work; Federation discovers and operates distributed capacity. Files, source
control, workspace search, and an interactive terminal are intentionally not Desktop
surfaces. Loopal performs workspace work through its governed Agent tools.

## Experience principles

- Keep one primary surface and preserve drafts when switching areas.
- Reveal operational detail only when Loopal produces meaningful state.
- Show identity, location, lifecycle, and freshness together.
- Prefer authoritative state over optimistic decoration.
- Keep Desktop chrome stable while Session and Agent projections evolve.

## Application frame

```text
+--------+------------------+---------------------------------------+
| Global | Session Sidebar  | Main Surface                          |
| Rail   | optional         | Conversation | Federation            |
|        |                  +---------------------------------------+
|        |                  | Context Dock (dynamic, optional)      |
+--------+------------------+---------------------------------------+
| Status Bar                                                       |
+------------------------------------------------------------------+
                         Settings overlay
```

### Global Rail

The Rail is the only permanent navigation. Conversation and Federation are product
areas. Attention is a counted interrupt; Sidebar and Settings are shell controls. The
macOS safe inset and blank rail/header regions are draggable, while controls remain
`no-drag`.

### Session Sidebar

Conversation uses the Sidebar for Session navigation and Session-level Federation
membership. Sessions are the user-facing root; working directories and Workspace IDs
remain internal execution/configuration scope and never group or filter navigation.
The default list shows every live Session across directories, ordered by recency.
Searching expands the catalog and separates matching live Sessions from history.
Federation is application-scoped and therefore owns no Session sidebar. Sidebar
visibility is independent of the selected product area.

Creating a Session always begins with an OS directory picker; no Workspace must be
selected first. A regular directory
runs directly. A directory inside a Git repository additionally offers an isolated
Worktree based on the current `HEAD`; the dialog names the new branch/worktree and
always explains that uncommitted changes are not copied. Cancel never creates a Session.

### Main Surface

Only one Main Surface is visible. Conversation carries Session and Agent identity;
Federation has no Workspace, Session, Agent, READY, Stop, or Restart toolbar. Switching
surfaces does not stop a Runtime, clear a draft, reset the selected Agent, or move focus.

### Context Dock

Conversation owns a horizontal segmented Dock below the transcript. It is absent when
no panel has meaningful content. One segment is open at a time; its height is bounded
and resizable. Counts summarize content, while alerts identify actionable failures.

### Settings overlay

Settings is a modal overlay rather than a product area. It keeps context visible, traps
focus, closes with Escape, and restores focus to its invoker. A searchable left navigation
groups second-level pages under Desktop, Loopal, Session, and Federation; only the selected
page is present on the right. Switching pages preserves unsaved drafts. Desktop, Loopal,
MCP, Agent, and MetaHub settings expose their exact scope. Loopal defaults and providers
write `~/.loopal/settings.json`; MCP stays scoped to the current Session directory.
Secrets are never echoed.

## Primary surfaces

### Conversation

Conversation is the default resumable home. Its stable order is toolbar, runtime status,
transcript, dynamic Dock, attention, and composer. The transcript renders text, thinking,
tools, images, errors, usage, compaction, cancellation, and streaming as one ordered event
narrative. Tool detail expands in place.

Selecting a child or remote Agent changes breadcrumb, transcript, composer, controls, and
Dock projection together. Questions, approvals, and permissions appear next to blocked
work and remain reachable from the Rail badge.

### Federation

Federation is an application-level operational overview. It contains coordinator health,
connection freshness, Hub filtering, Agent map/list, and selected-Agent detail. Filtering
never changes the active Session. “Open conversation” is available only for a routed
projection; “Manage federation” opens Settings.

Disconnected, connecting, connected, degraded, reconnecting, and error are distinct.
Healthy Hubs remain usable during partial failure. An empty Federation can start and
persist a managed local MetaHub without fabricating members.

`startLocalOnLaunch` restores only the application coordinator. `joinOnStart` independently
controls Runtime membership. Manual membership lives on each Session context menu and
resolves the exact live `(sessionId, runtimeId, generation)` target.

## State invariants

```text
area: conversation | federation
sidebar: hidden | visible
dock: absent | collapsed | open(panel, height)
settings: closed | open(section, scope)
```

- Settings overlays the current area without selecting a new area.
- Session changes select the root Agent, update internal directory scope, and bind
  commands to the new Runtime generation.
- Runtime is `starting | ready | running | waiting | stopping | stopped | failed`.
- Events from a retired generation cannot update transcript, topology, attention, or status.
- Focus remains under user control unless a focused element disappears or a modal opens.

## Dynamic panel rules

| Segment | Appears when | Primary interaction |
| --- | --- | --- |
| Agents | more than one local Agent exists | topology and Agent controls |
| Tasks | active Goal or incomplete task exists | progress and blockers |
| Background | at least one task is running | inspect and interrupt |
| Scheduled | at least one schedule exists | cadence and lifecycle |
| Artifacts | at least one artifact exists | preview and evidence |
| MCP | at least one MCP server is projected | health and diagnostics |
| Diagnostics | actionable Host, Runtime, Agent, or history issue | diagnose and recover |

New segments do not auto-expand while typing. If the open segment disappears, select the
nearest remaining segment while keeping the Dock collapsed. Each Session remembers segment,
collapse, and height.

## Remote identity and federation projection

The canonical remote identity is `(originHub, qualifiedAgentId, runtimeGeneration)`.
Display `name @ hub/path` plus locality, lifecycle, model, and freshness. Unknown, stale,
disconnected, and retired Agents remain inspectable but cannot receive controls.

The application controller aggregates generation-safe Runtime observations:

```text
FederationProjection
  connections[] -> owner Session/Runtime/generation, state, freshness, error
  hubs[]        -> qualified Hub identity, capabilities, connection sources
  agents[]      -> canonical identity, route, owner, lifecycle, conversation ref
```

Qualified identity controls deduplication. Conflicting or stale observations are marked
rather than overwritten. Commands resolve to one exact live owner generation.

## Responsive behavior

- `>=1280px`: Rail, Sidebar, Main Surface, and bounded Dock fit.
- `960–1279px`: Sidebar and toolbar become compact; Agent detail reflows.
- `720–959px`: Sidebar defaults closed; Dock tabs scroll horizontally.
- `<720px`: one surface remains visible and critical controls never overlap.

Resizing never resets selection or draft. Scrolling is preferable to overlapping controls.

## Keyboard contract

- `Cmd/Ctrl+1`: Conversation; `Cmd/Ctrl+2`: Federation.
- `Cmd/Ctrl+B`: Sidebar; `Cmd/Ctrl+K`: Session search.
- `Cmd/Ctrl+,`: Settings; `Cmd/Ctrl+Enter`: send.
- Dock tabs use Left/Right/Home/End.
- Escape closes the top modal or expanded layer.
- Shortcuts do not fire during IME composition.

## Verification contract

Observable acceptance belongs to the centralized
[Desktop E2E contract](./e2e-contract.md). It covers hidden/unfocused Electron behavior,
removed surfaces, directory-authorized Session creation, rich bilingual Conversation,
dynamic panels, real Loopal lifecycle, provider semantics, and generation-safe
Federation. This document remains the product-state contract rather than duplicating
suite mechanics or fixture details.

## Phased evolution

1. Shell coherence: spacing, toolbar hierarchy, drag regions, focus, and breakpoints.
2. Conversation fidelity: ordered events, Agent-scoped drafts, Dock, and attention.
3. Federation hardening: filtering, qualified identities, degraded states, and topology.
4. Application projection: provenance, freshness, deterministic routing, and cleanup.
5. General workbench: artifacts, routines, connectors, skills, and remote Runtimes.

Each phase ships with state-machine units, hidden Electron behavior, real Loopal lifecycle
coverage, responsive accessibility checks, and bilingual copy.
