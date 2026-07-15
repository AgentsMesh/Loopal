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

  it('drops a snapshot that completes after disposal', async () => {
    let release!: () => void
    const gate = new Promise<void>((resolve) => { release = resolve })
    const base = harness()
    const implementation = base.request.getMockImplementation()!
    base.request.mockImplementation(async (method, params, signal) => {
      if (method === 'view/snapshot') await gate
      return implementation(method, params, signal)
    })
    const pending = base.state.initialize()
    base.state.dispose()
    release()
    await pending
    expect(() => base.state.detail).toThrow('not ready')
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

  it('cancels pending invalidation timers on overflow and disposal', async () => {
    const { state, events } = harness()
    await state.initialize()
    state.accept('agent/event', event({ Stream: { text: 'live' } }, 3))
    state.accept('agent/event', event('AwaitingInput', 4))
    expect(events).toContainEqual(expect.objectContaining({
      type: 'conversation_entry', entry: expect.objectContaining({ text: 'live' }),
    }))
    state.accept('agent/event', event({ SessionHistoryLoaded: { messages: [] } }, 5))
    state.dispose()
    state.dispose()

    const direct = harness()
    await direct.state.initialize()
    direct.state.accept('agent/event', event({ SessionHistoryLoaded: { messages: [] } }, 3))
    direct.state.dispose()

    const failed = harness()
    await failed.state.initialize()
    const before = failed.request.mock.calls.length
    failed.request.mockRejectedValueOnce(new Error('timer refresh failed'))
    failed.state.accept('agent/event', event('Running', 3))
    await vi.waitFor(() => expect(failed.request.mock.calls.length).toBeGreaterThan(before))
  })

  it('contains explicit resync and trailing overflow refresh failures', async () => {
    const explicit = harness()
    await explicit.state.initialize()
    const before = explicit.request.mock.calls.length
    explicit.request.mockRejectedValueOnce(new Error('resync failed'))
    explicit.state.accept('view/resync_required', {})
    await vi.waitFor(() => expect(explicit.request.mock.calls.length).toBeGreaterThan(before))

    let release!: () => void
    const gate = new Promise<void>((resolve) => { release = resolve })
    const overflow = harness()
    const implementation = overflow.request.getMockImplementation()!
    let snapshots = 0
    overflow.request.mockImplementation(async (method, params, signal) => {
      if (method === 'view/snapshot' && ++snapshots === 1) await gate
      else if (method === 'view/snapshot') throw new Error('trailing refresh failed')
      return implementation(method, params, signal)
    })
    const pending = overflow.state.initialize()
    await vi.waitFor(() => expect(snapshots).toBe(1))
    for (let revision = 3; revision < 75; revision += 1) {
      overflow.state.accept('agent/event', event('Running', revision))
    }
    release()
    await expect(pending).rejects.toThrow('trailing refresh failed')
  })

  it('restarts a refresh invalidated at the drain boundary', async () => {
    const { state, events } = harness()
    const internals = state as unknown as {
      refresh(emit: boolean): Promise<void>
      runRefreshLoop(): Promise<void>
    }
    const drain = internals.runRefreshLoop.bind(state)
    let runs = 0
    internals.runRefreshLoop = async () => {
      runs += 1
      await drain()
      if (runs === 1) queueMicrotask(() => void internals.refresh(true))
    }
    await state.initialize()
    await vi.waitFor(() => expect(runs).toBe(2))
    await vi.waitFor(() => expect(events).toContainEqual(
      expect.objectContaining({ type: 'session_detail_replaced' }),
    ))
  })
})
