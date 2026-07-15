import { describe, expect, it, vi } from 'vitest'
import { CancellationToken } from '../../../../base/common/cancellation'
import { type DesktopEvent } from '../../../../shared/contracts'
import { createBackend } from './loopal-backend.test-fixtures'

describe('LoopalDesktopBackend workspace leader transitions', () => {
  it('publishes a fresh leader attached while the last leader is stopping', async () => {
    const { backend, hosts } = createBackend({
      sessionDirectoryRequest: async () => ({ path: '/workspace/project', name: 'project' }),
    })
    await backend.bootstrap()
    const events: DesktopEvent[] = []
    backend.onEvent((event) => events.push(event))
    let releaseStop!: () => void
    const stopGate = new Promise<void>((resolve) => { releaseStop = resolve })
    hosts[0]!.stop.mockImplementation(async () => {
      hosts[0]!.status('stopping')
      await stopGate
      hosts[0]!.status('stopped')
    })

    const stopping = backend.stopSession('session-1')
    await vi.waitFor(() => expect(hosts[0]!.currentStatus).toBe('stopping'))
    const selected = await backend.authorizeSessionDirectory('/workspace/project')
    const created = await backend.createSession({
      authorizationId: selected.authorizationId, launchMode: 'directory',
    })
    expect(created.session.id).toBe('session-3')
    expect(events.filter((event) => event.type === 'host_status').at(-1))
      .toEqual({ type: 'host_status', status: 'ready' })
    releaseStop()
    await stopping
    expect((await backend.bootstrap()).hostStatus).toBe('ready')
    await backend.gitStatus('local-workspace', CancellationToken.None)
    expect(hosts[1]!.request).toHaveBeenCalledWith(
      'workspace/gitStatus', { workspaceId: 'local-workspace' }, expect.any(AbortSignal),
    )
  })

  it('requires restart before a zero-live workspace becomes ready', async () => {
    const { backend, hosts, inputs } = createBackend()
    await backend.bootstrap()
    await backend.stopSession('session-1')
    const events: DesktopEvent[] = []
    backend.onEvent((event) => events.push(event))

    await expect(backend.gitStatus('local-workspace', CancellationToken.None))
      .rejects.toThrow('restart a session')
    await backend.restartSession('session-1')
    await backend.gitStatus('local-workspace', CancellationToken.None)
    expect(inputs.at(-1)).toEqual({
      workspaceId: 'local-workspace', cwd: '/workspace/project', resumeSessionId: 'session-1',
    })
    expect(events).toContainEqual({ type: 'host_status', status: 'ready' })
    const snapshot = await backend.bootstrap()
    expect(snapshot.hostStatus).toBe('ready')
    expect(snapshot.runtimes).toEqual([
      expect.objectContaining({ id: 'runtime-2', sessionId: 'session-1', state: 'ready' }),
    ])
    expect(hosts).toHaveLength(2)
  })
})
