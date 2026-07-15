import { type MetaHubRuntimeState } from '../../../../shared/contracts'
import { LoopalMetaHubWatcher } from './loopal-metahub-watcher'

describe('LoopalMetaHubWatcher', () => {
  it('coalesces starts, ignores equal refresh timestamps, and disposes its timer', async () => {
    vi.useFakeTimers()
    const current: MetaHubRuntimeState = {
      state: 'disconnected', hubs: [], topology: [], refreshedAt: '2026-01-01T00:00:00.000Z',
    }
    const request = vi.fn(async () => ({ agent_count: 1, uplink: null }))
    const changed = vi.fn(async () => undefined)
    const watcher = new LoopalMetaHubWatcher(
      { request } as never,
      () => new Date('2026-01-01T00:00:02.000Z'),
      () => current,
      changed,
    )
    watcher.start()
    watcher.start()
    await vi.advanceTimersByTimeAsync(1)
    expect(request).toHaveBeenCalledTimes(1)
    expect(changed).not.toHaveBeenCalled()
    watcher.dispose()
    await vi.advanceTimersByTimeAsync(2_100)
    expect(request).toHaveBeenCalledTimes(1)
    vi.useRealTimers()
  })

  it('contains refresh callback failures and keeps polling until disposal', async () => {
    vi.useFakeTimers()
    const request = vi.fn(async () => { throw new Error('old host') })
    const changed = vi.fn(async () => { throw new Error('render gone') })
    const watcher = new LoopalMetaHubWatcher(
      { request } as never, () => new Date(), () => undefined, changed,
    )
    watcher.start()
    await vi.advanceTimersByTimeAsync(1)
    expect(changed).toHaveBeenCalledOnce()
    await vi.advanceTimersByTimeAsync(2_001)
    expect(request).toHaveBeenCalledTimes(2)
    watcher.dispose()
    vi.useRealTimers()
  })

  it('does not revive after disposal while a poll is in flight', async () => {
    vi.useFakeTimers()
    let resolve!: (value: unknown) => void
    const request = vi.fn(() => new Promise((done) => { resolve = done }))
    const watcher = new LoopalMetaHubWatcher(
      { request } as never, () => new Date(), () => undefined, vi.fn(async () => undefined),
    )
    watcher.start()
    await vi.advanceTimersByTimeAsync(1)
    watcher.dispose()
    resolve({ agent_count: 0, uplink: null })
    await Promise.resolve()
    await vi.advanceTimersByTimeAsync(2_100)
    expect(request).toHaveBeenCalledOnce()
    vi.useRealTimers()
  })
})
