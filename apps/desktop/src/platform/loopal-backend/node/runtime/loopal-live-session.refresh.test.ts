import { describe, expect, it, vi } from 'vitest'
import {
  liveSessionEvent as event,
  liveSessionHarness as harness,
} from './loopal-live-session.test-fixtures'
import { LoopalSessionRefresh } from './loopal-session-refresh'

describe('LoopalLiveSession refresh', () => {
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
})

describe('LoopalSessionRefresh', () => {
  it('restarts work requested at the drain boundary and preserves emit intent', async () => {
    const emissions: boolean[] = []
    let refresh!: LoopalSessionRefresh
    refresh = new LoopalSessionRefresh(async (emit) => {
      emissions.push(emit)
      if (emissions.length === 1) {
        queueMicrotask(() => queueMicrotask(() => void refresh.request(true)))
      }
    })

    await refresh.request(false)
    await vi.waitFor(() => expect(emissions).toEqual([false, true]))
  })
})
