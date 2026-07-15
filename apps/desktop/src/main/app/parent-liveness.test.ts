import { monitorParent } from './parent-liveness'

describe('E2E parent liveness', () => {
  afterEach(() => vi.useRealTimers())

  it('quits once when the owning test process disappears', async () => {
    vi.useFakeTimers()
    const onMissing = vi.fn()
    const probe = vi.fn(() => { throw new Error('gone') })
    const monitor = monitorParent(42, onMissing, probe, 10)
    await vi.advanceTimersByTimeAsync(30)
    expect(probe).toHaveBeenCalledTimes(1)
    expect(onMissing).toHaveBeenCalledTimes(1)
    monitor.dispose()
  })

  it('stops probing when disposed', async () => {
    vi.useFakeTimers()
    const probe = vi.fn()
    const monitor = monitorParent(42, vi.fn(), probe, 10)
    await vi.advanceTimersByTimeAsync(10)
    monitor.dispose()
    await vi.advanceTimersByTimeAsync(30)
    expect(probe).toHaveBeenCalledTimes(1)
  })
})
