# Desktop E2E contract

Desktop E2E proves observable behavior across the real Electron security boundary.
Component details belong in units; packaging details belong in
[build and release](./build-and-release.md). This document is the single acceptance
contract referenced by [testing](./testing.md) and the
[experience model](./experience-model.md).

## Suite topology

`apps/desktop/desktop_tests.bzl` maps physical specs to three Bazel targets:

| Target | Specs | Runtime boundary |
| --- | --- | --- |
| `//apps/desktop:e2e` | `e2e/fake/**/*.spec.ts` | real Main/preload/Renderer with deterministic TypeScript backend |
| `//apps/desktop:e2e_host` | `e2e/real/host/**/*.spec.ts` | real Electron and Bazel-built Loopal Host |
| `//apps/desktop:e2e_llm_backend` | `e2e/real/provider/**/*.spec.ts` | real Host, production provider adapters, and Mock LLM |

Host and provider specs use feature subdirectories:

```text
e2e/real/host/{attention,federation}/
e2e/real/provider/{attention,federation}/
```

Shared harness code is grouped by responsibility:

```text
e2e/support/
  electron/     launch, window, application, and log control
  runtime/      real Host and Session helpers
  settings/     isolated Desktop/Loopal configuration
  fixtures/     copied workspaces and fixture path resolution
  providers/    provider-e2e and Mock LLM fixtures
  federation/   MetaHub and multi-Runtime helpers
```

Data-only fixtures remain under `e2e/fixtures/{llm,mcp,plugins,ssh,workspaces}` and are
Bazel runfiles.

## Common launch contract

- Each test receives isolated `HOME`, Electron `userData`, workspace, and runtime state.
- Electron runs with one Playwright worker so application and HOME lifecycles do not
  overlap.
- The BrowserWindow is created and painted but hidden and unfocused. Background
  throttling is disabled; macOS uses accessory activation policy.
- A dedicated test verifies hidden/unfocused behavior before and after relaunch while
  continuing to operate the Renderer.
- Normal application launches never enable hidden-E2E behavior.
- Trace, screenshot, and video are retained on failure.
- Tests assert DOM and authoritative protocol state; screenshots alone never pass a
  contract.

## Fake Electron contract

The fake suite exercises the real sandboxed Main, preload, MessagePort transport, and
Renderer. It verifies deterministic product behavior without launching Rust:

- exact preload surface, absent Node globals, and permission/navigation policy;
- Session catalog/search, creation UI, stop/restart generations, and draft retention;
- ordered Markdown conversation, thinking, tools, streaming, images, and Runtime state;
- Agent topology, attention, tasks, artifacts, MCP diagnostics, and dynamic panel rules;
- independent Federation surface, Session membership interactions, and degraded states;
- bilingual second-level Settings navigation, scopes, validation, focus, and persistence;
- keyboard behavior, IME safety, responsive layout, and title-bar drag/no-drag regions;
- explicit absence of Explorer, workspace search, SCM, editor/diff, standalone Worktree
  management, Terminal, xterm, and raw PTY preload methods.

Fake tests may use deterministic backend state, but they must not assert a Rust feature
that only the real Host can prove.

## Real Host contract

The Host suite launches the Bazel-built `loopal` executable with a temporary HOME and
repository-owned workspace. It proves:

- `alive` / UI registration / `session_created` / `ready` startup and typed Hub RPC;
- OS-authorized directory selection and one-shot directory capabilities;
- direct and tracked-subdirectory Worktree Sessions, pinned HEAD, clean source,
  stop/restart, and application relaunch;
- Session catalog and conversation recovery after complete relaunch;
- two live Host PIDs, message isolation, stop isolation, and restart generation/PID;
- bounded workspace operations, CAS writes, symlink rejection, and Host survival after
  oversized requests;
- rejection of removed raw PTY methods at the Host ACL;
- real permission/question/plan responses and retained child Agent conversations;
- local MetaHub lifecycle, Session join/leave, two-Runtime aggregation, rejoin, and stale
  generation rejection;
- graceful exit plus discovery/socket cleanup.

Provider-independent Host scenarios may still use the repository Mock LLM to obtain
deterministic Agent output, but Agent Server initialization remains the production path.

## Provider-boundary contract

Provider specs use declarative scenarios from `e2e/fixtures/llm` and the helpers in
`e2e/support/providers`. The same semantic scenario can exercise Anthropic, OpenAI
Responses, OpenAI-compatible Chat Completions, and Google streaming through production
adapters. Tests never set `LOOPAL_TEST_PROVIDER`.

Protocol matchers verify provider-specific headers and bodies. Request matchers cover
model, messages, images, tools/results, and history length. Scenarios cover:

- text/thinking streaming, signed thinking, usage, cache accounting, and finish reasons;
- client/server tools, parallel/progressive/failing tools, and continuation;
- pre-header and mid-stream faults, retries, cancellation, compaction, degeneration, and
  server-block recovery;
- Goals, plans, attention, subagents, background work, and topology transitions;
- MCP stdio, project Memory, provider Settings, and Session/provider-history relaunch;
- bidirectional MetaHub messages and remote running/completed Agent projections.

Every expected response must be consumed. Unmatched or in-flight requests, scenario
exhaustion, unresolved placeholders, and persisted credentials fail the test. The Mock
request journal is redacted and asserted together with Loopal Session projection and DOM.

## Fixture rules

- Workspace fixtures are copied; tests never mutate the repository fixture in place.
- Scenario strings may reference only `${PROJECT}`, `${HOME}`, and `${ROOT}`.
- Missing fixture files or unresolved placeholders fail instead of returning empty model
  output.
- Transport fixtures distinguish scripted close, client disconnect, pre-header failure,
  and exact semantic-stream disconnect positions.
- Secret values may enter isolated process environments but never snapshots, journals,
  Renderer state, or persisted settings assertions.

## Cross-layer acceptance

Federation acceptance intentionally spans units, fake Electron, and real Host/provider
tests. Units prove aggregation and command eligibility; fake E2E proves the independent
surface and interactions; real E2E proves actual Session Hosts, managed MetaHub state,
membership, isolation, and stale-generation rejection.

UI acceptance is complete only when English and Chinese labels, focus, hidden-window
operation, and generation-safe ownership agree with authoritative runtime state.
