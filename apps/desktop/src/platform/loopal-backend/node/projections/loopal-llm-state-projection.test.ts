import { vi } from 'vitest'
import { LoopalEventProjector } from './loopal-event-projector'
import { projectMessages } from './loopal-message-projection'
import { projectAgent, projectSessionView } from './loopal-view-projection'
import { ViewSnapshotSchema, type ViewSnapshot } from '../runtime/loopal-wire'

const now = new Date('2026-07-12T12:00:00.000Z')

describe('LLM authoritative Desktop projection', () => {
  it('projects server tools, tool failures, partial streams, and continuation state', () => {
    const snapshot = stateSnapshot()
    const entries = projectMessages(snapshot, now)
    const server = entries.find((entry) => entry.id === 'server-tool')?.toolCalls?.[0]
    const failed = entries.find((entry) => entry.id === 'failed-tool')?.toolCalls?.[0]

    expect(server).toMatchObject({
      id: 'search-1', name: 'web_search', status: 'succeeded',
      input: { query: 'Loopal Desktop' }, output: 'two verified sources',
    })
    expect(failed).toMatchObject({
      id: 'write-1', name: 'Write', status: 'failed', detail: 'permission denied',
    })
    expect(entries).toEqual(expect.arrayContaining([
      expect.objectContaining({
        role: 'system', text: 'Output truncated (max_tokens). Auto-continuing (1/8)',
      }),
      expect.objectContaining({
        role: 'system', text: expect.stringContaining('Context compacted (smart)'),
      }),
      expect.objectContaining({
        id: 'main-streaming-assistant', text: 'partial response before continuation',
        streaming: true,
      }),
    ]))
    expect(projectSessionView(snapshot)).toMatchObject({
      historyTruncated: true,
      retryBanner: 'Provider unavailable. Retrying in 2.0s (1/6)',
      compactBanner: '⠉ summarizing context — model request',
      streamingText: 'partial response before continuation',
    })
    expect(projectAgent(snapshot, undefined, now).telemetry).toMatchObject({
      inputTokens: 1_000, outputTokens: 200, cacheCreationTokens: 50,
      cacheReadTokens: 400, thinkingTokens: 80, toolsInFlight: 0, toolCount: 2,
    })
  })

  it('projects fatal errors and continuation governance notices through live events', () => {
    const append = vi.fn()
    const updateSession = vi.fn()
    const projector = new LoopalEventProjector(() => now, {
      append, updateSession, attention: vi.fn(),
    })
    projector.finishSync(0)
    projector.accept(event({ Stream: { text: 'partial before fatal error' } }, 1))
    projector.accept(event({ DegenerationDetected: {
      signal: 'barren_streak', count: 3,
    } }, 2))
    projector.accept(event({ ContinuationGateChanged: {
      open: false, closed_reason: 'idle_timeout', wake_deadline: '2026-07-12T13:00:00Z',
    } }, 3))
    projector.accept(event({ ContinuationGateChanged: { open: true } }, 4))
    projector.accept(event({ ContinuationSkipped: { reason: 'goal changed' } }, 5))
    projector.accept(event({ Error: { message: 'provider request failed fatally' } }, 6))

    expect(append.mock.calls.map(([entry]) => entry.text)).toEqual([
      'Degeneration detected: barren streak (3).',
      'Automatic continuation paused: idle timeout until 2026-07-12T13:00:00Z.',
      'Automatic continuation resumed.',
      'Continuation skipped: goal changed',
      'partial before fatal error',
      'provider request failed fatally',
    ])
    expect(append.mock.calls.slice(0, 4).every(([entry]) => entry.eventNotice)).toBe(true)
    expect(append).toHaveBeenLastCalledWith(expect.objectContaining({ role: 'error' }))
    expect(updateSession).toHaveBeenLastCalledWith('failed', 'failure')
  })
})

function stateSnapshot(): ViewSnapshot {
  return ViewSnapshotSchema.parse({
    rev: 9,
    state: { agent: {
      name: 'main', observable: { status: 'Running', model: 'claude-opus-4-8' },
      conversation: {
        history_truncated: true,
        streaming_text: 'partial response before continuation',
        retry_banner: 'Provider unavailable. Retrying in 2.0s (1/6)',
        compact_banner: '⠉ summarizing context — model request',
        turn_count: 2, input_tokens: 1_000, output_tokens: 200, context_window: 200_000,
        cache_creation_tokens: 50, cache_read_tokens: 400, thinking_tokens: 80,
        messages: [
          message('server-tool', tool('search-1', 'web_search', {
            state: 'done', outcome: { type: 'success', content: 'two verified sources' },
          }, { query: 'Loopal Desktop' })),
          message('failed-tool', tool('write-1', 'Write', {
            state: 'done', outcome: { type: 'failure', error: 'permission denied' },
          }, { file_path: 'denied.txt' })),
          { role: 'system', content: 'Output truncated (max_tokens). Auto-continuing (1/8)' },
          { role: 'system', content: 'Context compacted (smart): 8→2 messages (6 summarized), 12000→3000 tokens (75% freed).' },
        ],
      },
    } },
  })
}

function message(id: string, invocation: unknown) {
  return { message_id: id, role: 'assistant', content: '', tool_calls: [invocation] }
}

function tool(id: string, name: string, state: object, input: object) {
  return { id, name, summary: `${name}(fixture)`, input, state }
}

function event(payload: unknown, rev: number) {
  return { agent_name: { hub: [], agent: 'main' }, event_id: rev, rev, payload }
}
