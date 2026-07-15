# LoopalDesktop E2E fixtures

`workspaces/basic` is copied into a fresh temporary project for every Electron test. The test
initializes its own Git repository after the copy, so source contents stay stable while runtime
state remains isolated.

Session-directory E2E also copies `workspaces/basic` into test-owned paths with and without Git.
The real Desktop selects those paths through an opaque main-process authorization, then verifies
direct and worktree Sessions from the Loopal process itself. Native pickers are replaced only in
hidden E2E mode by a queued test-owned selection, so the suite never takes window focus.

`workspaces/memory` adds a private `.loopal/memory` graph and disabled-by-default Memory setting.
Memory E2E copies it into the same temporary boundary; it never reads or writes the repository's
real `.loopal/memory` directory.

`plugins/global-skills` is copied only into the isolated E2E HOME. It contributes instructions,
a disabled MCP declaration, and `/plugin-check`, allowing Settings to verify plugin inventory
without executing an external process or touching the developer's global Loopal configuration.
The global Skill test creates its editable Skill through Settings instead of seeding that file.

`llm/*.json` contains declarative scenarios consumed by `//crates/loopal-mock-llm`. Scenario v2
drives Anthropic, OpenAI Responses, OpenAI-compatible Chat Completions, and Google streaming from
the same semantic chunks. A scenario has a version, name, ordered or request-matched calls, and an
optional fallback.

Supported request matchers are `protocol`, `model`, `userContains`, `bodyContains`,
`bodyExcludes`, `toolResultId`, `minTools`, `messageCount`, `thinkingEnabled`, and
`imageBlockCount`. Exact semantic history matching also supports `assistantBlockTypes` and
`serverBlockCount`. `protocol` accepts `anthropic`,
`openai_responses`, `openai_compat`, or `google`; conditional matching also supports concurrent
Agent requests.
Supported response fields include `status`, `retryAfterMs`, `delayMs`, `headers`, `body`,
`chunks`, `rawSse`, `disconnectAfterEvents`, and `closeBeforeHeaders`.

Stream chunks support text, thinking, thinking signatures, tool use, server tool use/result,
usage, delays, explicit stop reasons, malformed SSE, and disconnects.
`pause_turn` is Anthropic-only; cross-provider terminal fixtures use `max_tokens`.
Scenario parsing rejects unknown fields and invalid chunk shapes before the server binds. The
control state tracks unmatched requests, in-flight responses, `clientDisconnects`, and
`scriptedDisconnects`. `closeBeforeHeaders`, `disconnectAfterEvents`, and semantic `disconnect`
chunks count as scripted transport failures; cancellation by Loopal counts as a client disconnect.
`llm/desktop-demo.json` is the reusable manual smoke scenario for a locally launched Desktop.
Provider E2E also covers native OpenAI/Google server search history and an OpenAI
Settings save, Session restart, authenticated request, and rendered response. Repo-owned fixtures
also drive a real stdio MCP process, Memory maintainer agents, Electron relaunch recovery, and
bidirectional MetaHub plus cross-Hub Agent lifecycles without external services.

`GET /__mock/requests` exposes a protocol-neutral, bounded journal for contract assertions without
storing credentials, request headers, query keys, complete bodies, or tool-result content.
`GET /__mock/state` and `GET /__mock/verify` report consumption and verification state.

String values may use `${PROJECT}`, `${HOME}`, or `${ROOT}`. The fixture loader substitutes only
these test-owned paths before starting the backend and rejects unresolved placeholders.

Run the provider boundary suite with:

```sh
bazel test //apps/desktop:e2e_llm_backend --test_output=errors
```

Electron E2E creates and paints the real BrowserWindow in hidden, unfocused test-only mode. It
disables background throttling, uses the macOS accessory activation policy, and never changes the
behavior of a normal visible Desktop launch.
