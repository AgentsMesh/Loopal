import { describe, expect, it, vi } from 'vitest'
import { type DesktopEvent } from '../../../../shared/contracts'
import { agentEvent, createBackend } from './loopal-backend.test-fixtures'

describe('LoopalDesktopBackend scoped events', () => {
  it('bounds a slow snapshot burst and schedules a trailing authoritative refresh', async () => {
    const { backend, hosts } = createBackend()
    await backend.bootstrap()
    const host = hosts[0]!
    const replacements: DesktopEvent[] = []
    backend.onEvent((event) => {
      if (event.type === 'session_detail_replaced') replacements.push(event)
    })
    const base = host.request.getMockImplementation()!
    let release!: () => void
    const gate = new Promise<void>((resolve) => { release = resolve })
    let snapshots = 0
    host.request.mockImplementation(async (method, params, signal) => {
      if (method === 'view/snapshot' && ++snapshots === 1) await gate
      return base(method, params, signal)
    })
    host.notification('view/resync_required', {})
    await vi.waitFor(() => expect(snapshots).toBe(1))
    for (let revision = 3; revision < 80; revision += 1) {
      host.notification('agent/event', agentEvent({ Stream: { text: `${revision}` } }, revision, revision))
    }
    host.snapshotRevision = 100
    host.snapshotContent = 'authoritative after overflow'
    release()
    await vi.waitFor(() => expect(snapshots).toBe(4))
    await vi.waitFor(() => expect(replacements.at(-1)).toMatchObject({
      detail: { conversation: [expect.objectContaining({ text: 'authoritative after overflow' })] },
    }))
  })
})
