import { describe, expect, it, vi } from 'vitest'
import { SessionRuntimeEntry } from './session-runtime-registry-entry'
import { FakeRuntimeHost } from './session-runtime-registry.test-fixtures'

describe('SessionRuntimeEntry', () => {
  it('overflows from a pre-bind status and emits one scoped resync', () => {
    const host = new FakeRuntimeHost('session')
    const status = vi.fn()
    const notification = vi.fn()
    const crashed = vi.fn()
    const entry = new SessionRuntimeEntry(
      'workspace', '/workspace', 'runtime', 3, host, undefined,
      { status, notification, crashed },
    )
    expect(entry.resumeInput()).toBeUndefined()
    for (let index = 0; index < 64; index += 1) {
      host.notify('agent/event', { index })
    }
    host.crash()
    const handle = entry.bindSession('session')
    expect(handle).toMatchObject({ sessionId: 'session', runtimeId: 'runtime', generation: 3 })
    expect(status).toHaveBeenCalledWith(expect.objectContaining({ status: 'crashed' }))
    expect(notification).toHaveBeenCalledWith(expect.objectContaining({
      method: 'view/resync_required', params: { reason: 'pre_ready_buffer_overflow' },
    }))
    expect(crashed).toHaveBeenCalledOnce()
    entry.dispose()
    host.notify('agent/event', { late: true })
    expect(notification).toHaveBeenCalledOnce()
  })
})
