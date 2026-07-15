import { describe, expect, it, vi } from 'vitest'
import { CancellationToken } from '../../../../base/common/cancellation'
import { type DesktopEvent } from '../../../../shared/contracts'
import { agentEvent, createBackend, timestamp } from './loopal-backend.test-fixtures'
import { nativeTestPath } from './loopal-backend.test-paths'

describe('LoopalDesktopBackend scoped events', () => {
  it('routes messages and equal revisions independently across two sessions', async () => {
    const { backend, hosts } = createBackend()
    await backend.bootstrap()
    await backend.restartSession('session-2')
    const events: DesktopEvent[] = []
    backend.onEvent((event) => events.push(event))

    const image = {
      name: 'pixel.png', mediaType: 'image/png' as const, data: 'iVBORw==', sizeBytes: 4,
    }
    await backend.sendMessage('session-1', 'message one', CancellationToken.None, 'main', [image])
    await backend.sendMessage('session-2', 'message two', CancellationToken.None, 'worker')
    expect(hosts[0]!.request).toHaveBeenCalledWith('hub/route', expect.objectContaining({
      content: {
        text: 'message one', images: [{ media_type: 'image/png', data: 'iVBORw==' }],
      },
    }))
    expect(hosts[1]!.request).toHaveBeenCalledWith('hub/route', expect.objectContaining({
      target: { hub: [], agent: 'worker' },
      content: { text: 'message two', images: [] },
    }))
    expect(events).toContainEqual({
      type: 'conversation_entry', sessionId: 'session-1',
      entry: expect.objectContaining({ role: 'user', imageCount: 1 }),
    })

    hosts[0]!.notification('agent/event', agentEvent({ Stream: { text: 'first' } }, 3, 3))
    hosts[0]!.notification('agent/event', agentEvent('AwaitingInput', 4, 4))
    hosts[1]!.notification('agent/event', agentEvent({ Stream: { text: 'second' } }, 3, 3))
    hosts[1]!.notification('agent/event', agentEvent({ TurnCompleted: {} }, 4, 4))
    expect(events).toContainEqual({
      type: 'conversation_entry', sessionId: 'session-1',
      entry: expect.objectContaining({ text: 'first' }),
    })
    expect(events).toContainEqual({
      type: 'conversation_entry', sessionId: 'session-2',
      entry: expect.objectContaining({ text: 'second' }),
    })
    expect(events.filter((event) => event.type === 'session_updated')).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ session: expect.objectContaining({ id: 'session-1', status: 'waiting' }) }),
        expect.objectContaining({ session: expect.objectContaining({ id: 'session-2', status: 'running' }) }),
      ]),
    )
  })
  it('scopes attention to a generation, clears it on retire, and rejects stale responses', async () => {
    const { backend, hosts } = createBackend()
    await backend.bootstrap()
    const events: DesktopEvent[] = []
    backend.onEvent((event) => events.push(event))
    hosts[0]!.notification('agent/event', agentEvent({
      ToolPermissionRequest: { id: 'permission', name: 'Bash', input: { command: 'pwd' } },
    }, 3, 3))
    expect(events).toContainEqual({
      type: 'permission_requested',
      request: expect.objectContaining({
        id: 'permission', sessionId: 'session-1', runtimeId: 'runtime-1', generation: 1,
      }),
    })
    await backend.respondPermission({
      sessionId: 'session-1', runtimeId: 'runtime-1', generation: 1,
      agentId: 'main', requestId: 'permission', decision: 'allow_once',
    }, CancellationToken.None)

    const restarted = await backend.restartSession('session-1')
    expect(restarted).toMatchObject({ id: 'runtime-2', generation: 2 })
    expect(events).toContainEqual({
      type: 'permission_resolved', sessionId: 'session-1',
      runtimeId: 'runtime-1', generation: 1, agentId: 'main', requestId: 'permission',
    })
    expect(events.filter((event) => event.type === 'session_updated').at(-1)).toEqual({
      type: 'session_updated',
      session: expect.not.objectContaining({ attention: expect.anything() }),
    })
    const count = hosts.length
    await expect(backend.respondPermission({
      sessionId: 'session-1', runtimeId: 'runtime-1', generation: 1,
      agentId: 'main', requestId: 'permission', decision: 'deny',
    }, CancellationToken.None)).rejects.toMatchObject({ code: 'RUNTIME_GONE' })
    expect(hosts).toHaveLength(count)

    const before = events.length
    hosts[0]!.notification('agent/event', agentEvent({ Stream: { text: 'late old host' } }, 9, 9))
    hosts[0]!.notification('agent/event', agentEvent('AwaitingInput', 10, 10))
    expect(events).toHaveLength(before)

    await backend.stopSession('session-1')
    const stoppedCount = hosts.length
    await expect(backend.respondPermission({
      sessionId: 'session-1', runtimeId: 'runtime-2', generation: 2,
      agentId: 'main', requestId: 'stopped', decision: 'deny',
    }, CancellationToken.None)).rejects.toMatchObject({ code: 'RUNTIME_GONE' })
    expect(hosts).toHaveLength(stoppedCount)
  })
  it('uses one workspace leader and reopens the most recent session after zero live Hosts', async () => {
    let tick = 0
    const { backend, hosts, inputs } = createBackend({
      now: () => new Date(timestamp.getTime() + tick++ * 1_000),
    })
    await backend.bootstrap()
    await backend.restartSession('session-2')
    const events: DesktopEvent[] = []
    backend.onEvent((event) => events.push(event))

    hosts[1]!.notification('workspace/fileChanged', {
      workspaceId: 'local-workspace', path: 'ignored.rs', kind: 'changed',
    })
    hosts[0]!.notification('workspace/fileChanged', {
      workspaceId: 'local-workspace', path: 'leader.rs', kind: 'changed',
    })
    expect(events).not.toContainEqual(expect.objectContaining({ path: 'ignored.rs' }))
    expect(events).toContainEqual(expect.objectContaining({ type: 'file_changed', path: 'leader.rs' }))

    let releaseStop!: () => void
    const stopGate = new Promise<void>((resolve) => { releaseStop = resolve })
    hosts[0]!.stop.mockImplementation(async () => {
      hosts[0]!.status('stopping')
      await stopGate
      hosts[0]!.status('stopped')
    })
    const stopping = backend.stopSession('session-1')
    await vi.waitFor(() => expect(hosts[0]!.currentStatus).toBe('stopping'))
    hosts[0]!.notification('workspace/fileChanged', {
      workspaceId: 'local-workspace', path: 'retiring.rs', kind: 'changed',
    })
    hosts[1]!.notification('workspace/fileChanged', {
      workspaceId: 'local-workspace', path: 'next.rs', kind: 'changed',
    })
    await backend.gitStatus('local-workspace', CancellationToken.None)
    expect(events).not.toContainEqual(expect.objectContaining({ path: 'retiring.rs' }))
    expect(events).toContainEqual(expect.objectContaining({ type: 'file_changed', path: 'next.rs' }))
    expect(hosts[1]!.request).toHaveBeenCalledWith(
      'workspace/gitStatus', { workspaceId: 'local-workspace' }, expect.any(AbortSignal),
    )
    releaseStop()
    await stopping
    await backend.stopSession('session-2')
    await expect(backend.gitStatus('local-workspace', CancellationToken.None))
      .rejects.toThrow('restart a session')
    await backend.restartSession('session-2')
    await expect(backend.gitStatus('local-workspace', CancellationToken.None))
      .resolves.toMatchObject({ branch: 'main' })
    expect(inputs.at(-1)).toEqual({
      workspaceId: 'local-workspace', cwd: nativeTestPath('/workspace/project'),
      resumeSessionId: 'session-2',
    })
  })

})
