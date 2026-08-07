import { describe, expect, it, vi } from 'vitest'
import {
  liveSessionEvent as event, liveSessionHarness as harness,
  liveSessionSummary as summary,
} from './loopal-live-session.test-fixtures'

describe('LoopalLiveSession', () => {
  it('guards unavailable detail and inactive sessions', async () => {
    const { state, request, events, summaries } = harness()
    expect(() => state.detail).toThrow('not ready')
    state.replaceSummary({ ...summary, title: 'Before snapshot' })
    state.accept('unknown', {})
    await state.send('before detail')
    expect(request).toHaveBeenCalledWith('hub/route', expect.anything())
    expect(events).toEqual([])
    expect(summaries).toHaveLength(1)
    state.dispose()
    state.accept('agent/event', event('Running', 3))
    await state.initialize()
    expect(() => state.detail).toThrow('not ready')
  })

  it('updates initialized detail, tracks both attention kinds, and resolves them on retire', async () => {
    const { state, request, events, summaries } = harness()
    await state.initialize()
    const next = { ...summary, title: 'Renamed' }
    state.replaceSummary(next)
    expect(state.detail.session.title).toBe('Renamed')
    await state.send('hello')
    expect(events).toContainEqual({
      type: 'conversation_entry', sessionId: 'session',
      entry: expect.objectContaining({ role: 'user', text: 'hello' }),
    })
    expect(summaries.at(-1)).toMatchObject({ status: 'running' })
    state.detail.agents.push({
      id: 'worker', name: 'Worker', parentId: 'main', status: 'waiting',
    })
    await state.send('hello child', 'worker')
    expect(request).toHaveBeenCalledWith('hub/route', expect.objectContaining({
      target: { hub: [], agent: 'worker' },
    }))
    expect(state.detail.agents.find((agent) => agent.id === 'worker')?.conversation)
      .toContainEqual(expect.objectContaining({ text: 'hello child', agentId: 'worker' }))
    expect(events).toContainEqual(expect.objectContaining({ type: 'session_detail_replaced' }))

    state.accept('agent/event', event({
      ToolPermissionRequest: { id: 'permission', name: 'Read', input: 'README.md' },
    }, 3))
    state.accept('agent/event', event({
      UserQuestionRequest: {
        id: 'question', questions: [{ question: 'Continue?', options: [], allow_multiple: false }],
      },
    }, 4))
    state.accept('agent/event', event({ ToolPermissionResolved: { id: 'permission' } }, 5))
    state.accept('agent/event', event({ UserQuestionResolved: { id: 'question' } }, 6))
    state.accept('agent/event', event({
      UserQuestionRequest: {
        id: 'pending', questions: [{ question: 'Again?', options: [], allow_multiple: false }],
      },
    }, 7))
    state.accept('agent/event', event({ ToolPermissionRequest: {} }, 8))
    expect(state.retire()).toEqual([{
      type: 'question_resolved', sessionId: 'session', runtimeId: 'runtime',
      generation: 1, agentId: 'main', requestId: 'pending',
    }])
    expect(state.retire()).toEqual([])
    const summaryCount = summaries.length
    await state.send('late')
    await state.send('late child', 'worker')
    expect(summaries).toHaveLength(summaryCount)
  })

  it('applies snapshot attention before replaying buffered live events', async () => {
    let release!: () => void
    const gate = new Promise<void>((resolve) => { release = resolve })
    const value = harness()
    const implementation = value.request.getMockImplementation()!
    value.request.mockImplementation(async (method, params, signal) => {
      if (method === 'view/snapshot') await gate
      return implementation(method, params, signal)
    })
    const pending = value.state.initialize()
    await vi.waitFor(() => expect(value.request).toHaveBeenCalledWith(
      'view/snapshot', { agent: 'main' },
    ))
    value.state.accept('agent/event', event({
      UserQuestionRequest: {
        id: 'during-sync',
        questions: [{ question: 'Continue?', options: [], allow_multiple: false }],
      },
    }, 3))

    release()
    await pending

    expect(value.events).toContainEqual(expect.objectContaining({
      type: 'question_requested',
      request: expect.objectContaining({ id: 'during-sync' }),
    }))
    expect(value.events).not.toContainEqual(expect.objectContaining({
      type: 'question_resolved', requestId: 'during-sync',
    }))
    expect(value.state.retire()).toContainEqual(expect.objectContaining({
      type: 'question_resolved', requestId: 'during-sync',
    }))
  })

  it('replays a newer lifecycle event over a running startup snapshot', async () => {
    let release!: () => void
    const gate = new Promise<void>((resolve) => { release = resolve })
    const value = harness()
    const implementation = value.request.getMockImplementation()!
    value.request.mockImplementation(async (method, params, signal) => {
      if (method === 'view/snapshot') {
        await gate
        return {
          rev: 2,
          state: { agent: {
            name: 'main', observable: { status: 'Running' },
            conversation: { streaming_text: '', messages: [] },
          } },
        }
      }
      return implementation(method, params, signal)
    })

    const pending = value.state.initialize()
    await vi.waitFor(() => expect(value.request).toHaveBeenCalledWith(
      'view/snapshot', { agent: 'main' },
    ))
    value.state.accept('agent/event', event('AwaitingInput', 3))
    release()
    await pending

    expect(value.state.detail.agents.find((agent) => agent.id === 'main')?.status).toBe('waiting')
  })

  it('projects ordered lifecycle events without waiting for a snapshot refresh', async () => {
    const value = harness()
    await value.state.initialize()
    value.state.detail.agents.push({
      id: 'worker', name: 'Worker', parentId: 'main', status: 'waiting',
    })

    value.state.accept('agent/event', event('Running', 3))
    value.state.accept('agent/event', event('Running', 3, 'worker'))
    expect(value.state.detail.agents.find((agent) => agent.id === 'main')?.status).toBe('running')
    expect(value.state.detail.agents.find((agent) => agent.id === 'worker')?.status).toBe('running')

    value.state.accept('agent/event', event('AwaitingInput', 4))
    value.state.accept('agent/event', event({ Error: { message: 'child failed' } }, 4, 'worker'))
    value.state.accept('agent/event', event('Finished', 5, 'worker'))
    expect(value.state.detail.agents.find((agent) => agent.id === 'main')?.status).toBe('waiting')
    expect(value.state.detail.agents.find((agent) => agent.id === 'worker')?.status).toBe('failed')

    value.state.accept('agent/event', event('Running', 3))
    expect(value.state.detail.agents.find((agent) => agent.id === 'main')?.status).toBe('waiting')
  })

  it('retains deduplicated artifacts produced by root and child turns', async () => {
    const { state, events } = harness()
    await state.initialize()
    state.accept('agent/event', event({
      TurnDiffSummary: { modified_files: ['./src/main.rs', 'src/main.rs', 'README.md'] },
    }, 3, 'worker'))
    expect(state.detail.artifacts).toHaveLength(2)
    expect(state.detail.artifacts[0]).toMatchObject({
      title: 'main.rs', kind: 'code', producerAgentId: 'worker',
    })
    expect(events.filter((item) => item.type === 'artifact_created')).toHaveLength(2)
    state.accept('agent/event', event({
      TurnDiffSummary: { modified_files: ['src/main.rs'] },
    }, 4, 'worker'))
    expect(state.detail.artifacts).toHaveLength(2)
  })

})
