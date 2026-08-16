# Loopal Mock LLM

Bazel-only wire mock for Loopal provider and Desktop E2E tests. One semantic
scenario can drive every supported provider without embedding provider-specific
SSE in fixtures.

```sh
bazel run //crates/loopal-mock-llm:loopal-mock-llm -- \
  --scenario apps/desktop/e2e/fixtures/llm/default-ok.json \
  --api-key test-key
```

The first stdout line contains the random loopback `baseUrl`. Use the same
origin for each provider:

- Anthropic: `POST /v1/messages`, `x-api-key`, exact `anthropic-version`.
- OpenAI Responses: `POST /v1/responses`, `Authorization: Bearer`.
- OpenAI-compatible: `POST /v1/chat/completions`, `Authorization: Bearer`.
- Google: `POST /models/{model}:streamGenerateContent?alt=sse&key=...`.
  `/v1beta/models/...` is also accepted when the configured base includes it.

Scenario v2 adds an optional protocol matcher while preserving v1 and the
global conditional-call scan used by concurrent Agent requests. Matcher values
are `anthropic`, `openai_responses`, `openai_compat`, and `google`:

```json
{
  "version": 2,
  "calls": [{
    "expect": {
      "protocol": "openai_responses",
      "model": "test-model",
      "userContains": "contract marker",
      "minTools": 1,
      "thinkingEnabled": true,
      "imageBlockCount": 0
    },
    "chunks": [
      {"type": "thinking", "text": "checking"},
      {"type": "text", "text": "ready"},
      {
        "type": "tool_use",
        "id": "read-1",
        "name": "Read",
        "input": {"file_path": "README.md"}
      },
      {"type": "usage", "input": 12, "output": 7, "thinking": 3},
      {"type": "done", "reason": "end_turn"}
    ]
  }]
}
```

Scenario v3 adds optional call labels and request metadata predicates. Metadata
paths are JSON Pointers relative to the top-level request `metadata` value. All
predicates in `requestMetadata` must match:

```json
{
  "version": 3,
  "calls": [{
    "label": "build node attempt 2",
    "expect": {"requestMetadata": [
      {"path": "/workflow/run", "exists": true},
      {"path": "/workflow/node", "equals": "build"},
      {"path": "/workflow/attempt", "equals": 2},
      {"path": "/workflow/phase", "contains": "execute"},
      {"path": "/workflow/phase", "excludes": "cancel"}
    ]},
    "chunks": [{"type": "done"}]
  }]
}
```

Labels and matcher paths appear in mismatch diagnostics. Request metadata values
and the metadata object are never copied into the request journal.
`assistantBlockTypes` matches the exact canonical assistant history order;
`serverBlockCount` matches the number of canonical server-history blocks.

Fixtures live under `apps/desktop/e2e/fixtures/llm` and enter tests as Bazel
runfiles. `GET /__mock/requests` returns a protocol-neutral, bounded journal. It records model,
message/tool counts, tool names and result IDs, error result IDs, block counts,
image counts, last user text, system/thinking/stream flags and token limits. Credentials,
headers, query keys, tool-result content and full request bodies are never
stored. The `verified` field from `GET /__mock/state` or `GET /__mock/verify` is
true only when all calls matched and no response is in flight; state also exposes
unmatched request and disconnect counters.

Desktop contracts use this journal to lock native OpenAI/Google search replay order and the
Settings-save → Session-restart → authenticated OpenAI request path.

`pause_turn` is an Anthropic terminal; portable terminal scenarios use
`max_tokens`, which each renderer maps to its provider-specific wire reason.

Transport controls are deterministic:

- `closeBeforeHeaders: true` closes TCP before any HTTP response.
- `disconnectAfterEvents` closes after a chosen SSE event count.
- `{"type":"disconnect"}` closes at an exact semantic stream position.
- `delayMs` and `{"type":"delay","ms":...}` support cancellation tests.

Scripted closes increment `scriptedDisconnects`; client cancellation increments
`clientDisconnects`.
