# Desktop slash commands

Slash text is a client interaction syntax. It is not a second Runtime protocol and
must not bypass the typed Desktop boundary. Runtime `ControlCommand` remains the
canonical control-plane representation; ordinary prompts and Skills remain on the
data plane.

## Dispatch model

```text
composer input
  |-- ordinary text ----------> sendMessage -> hub/route
  |-- known Runtime command --> controlAgent -> hub/control -> applied + events
  |-- effective Skill --------> sendMessage -> hub/route -> Hub expands Skill
  |-- local UI command -------> Renderer behavior only
  `-- unknown /text ----------> sendMessage -> hub/route
```

The last branch deliberately matches the TUI: an unknown slash prefix can still be
a legitimate prompt. A syntactically invalid known command is different. It remains
in the composer, displays an actionable error, and sends neither a control request
nor an LLM request.

## Command catalog

The Desktop catalog describes each command with a stable name, localized description,
usage, argument policy, and transport. Runtime commands are limited to semantics that
already have a typed `AgentControlCommand`:

- `/act`, `/plan`, `/clear`, `/suspend`, and `/unsuspend` take no arguments.
- `/compact [instructions]` accepts optional instructions.
- `/model <model>` and `/rewind <turn-index>` require one value.
- `/permission <bypass|ask_dangerous|ask_any_write>` selects permission policy.
- `/decision <manual|classifier|agent>` selects decision policy.
- `/sandbox <disabled|default_write|read_only>` selects sandbox policy.
- `/mcp [status|reconnect <server>|disconnect <server>]` controls MCP state.
- `/help` is local and opens the command catalog.

Process commands such as TUI `/exit`, `/detach-hub`, and `/kill-hub` are intentionally
not exposed: Desktop owns process and federation lifecycle through separate typed
operations. Picker-heavy or side-effecting TUI commands can be added only after their
Desktop workflow and protocol are explicit.

## Skills

The Renderer queries `desktop/listSkills` for the selected Workspace and includes only
effective Skills in completion. Built-ins reserve their names when a Skill collides.
Selecting a Skill inserts its slash form, but submission uses the normal message path.
The routing Hub reloads the authorized effective configuration, expands `$ARGUMENTS`,
and records the invocation. The Renderer never expands or persists a Skill body.

This keeps completion responsive without turning a cached list into execution
authority. A future strict `hub/routeSkill` may bind a selected Skill revision if the
configuration race needs stronger guarantees.

## Composer behavior

- Completion opens only for a slash token at the start of the draft.
- Filtering covers command name and localized description.
- Arrow keys move, Tab completes, Enter selects or submits, and Escape closes.
- IME composition never triggers selection or submission.
- The popup overlays the transcript and does not resize it.
- Images are valid only for the data plane; known control commands reject attachments.
- The draft clears only after a local action completes, `hub/control` is applied, or
  `hub/route` accepts the message.
- Authoritative Runtime events update mode, policy, model, clearing, rewind, and
  compaction state. The Renderer does not fabricate success receipts.

## Trust boundary

The command parser is an interaction adapter, not an authorization layer. Main validates
every `AgentControlCommand`, resolves the exact `(sessionId, runtimeId, generation,
agentId)` target, rejects stale generations, forwards `hub/control`, and waits for the
Runtime acknowledgement. Skill execution remains subject to Hub routing, Runtime policy,
and the target Workspace configuration.

## Verification

Unit tests cover parsing, required and optional arguments, enum validation, built-in
collision precedence, keyboard behavior, IME handling, draft retention, and bilingual
copy. Hidden Electron and real-Hub acceptance belongs to the centralized
[E2E contract](./e2e-contract.md); slash scenarios prove `/plan`, permission changes,
invalid arguments without an LLM request, ordinary prompts with one request, and a
fixture Skill whose body is expanded only by the real Hub.
