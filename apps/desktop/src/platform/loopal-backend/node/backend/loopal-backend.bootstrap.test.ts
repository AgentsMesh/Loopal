import { describe, expect, it, vi } from 'vitest'
import { createBackend, timestamp } from './loopal-backend.test-fixtures'
import { nativeTestFileUri, nativeTestPath } from './loopal-backend.test-paths'

describe('LoopalDesktopBackend bootstrap and lifecycle', () => {
  it('boots protocol v2 with a live waiting session and stopped catalog sessions', async () => {
    const { backend, hosts } = createBackend()
    const bootstrap = await backend.bootstrap()

    expect(bootstrap).toMatchObject({
      protocolVersion: 2,
      hostStatus: 'ready',
      activeSessionId: 'session-1',
      workspaces: [{
        id: 'local-workspace', name: 'project',
        rootUri: nativeTestFileUri('/workspace/project'), kind: 'folder',
      }],
    })
    expect(bootstrap.sessions).toEqual([
      expect.objectContaining({
        id: 'session-1', title: 'Raw session-1', status: 'waiting',
        activeRuntimeId: 'runtime-1',
      }),
      expect.objectContaining({
        id: 'session-2', title: 'Raw session-2', status: 'stopped',
      }),
    ])
    expect(bootstrap.runtimes).toEqual([{
      id: 'runtime-1', sessionId: 'session-1', workspaceId: 'local-workspace',
      generation: 1, state: 'ready', rootAgent: 'main', startedAt: timestamp.toISOString(),
    }])
    const detail = await backend.openSession('session-1')
    expect(detail.conversation).toEqual([expect.objectContaining({
      role: 'assistant', text: 'Answer from session-1',
    })])
    expect(detail.agents).toEqual([
      expect.objectContaining({ id: 'main', name: 'Loopal', status: 'waiting' }),
      expect.objectContaining({
        id: 'worker', name: 'worker', status: 'waiting', parentId: 'main',
      }),
    ])
    expect(hosts[0]!.request).toHaveBeenCalledWith('desktop/listSessions', {
      workspaceId: 'local-workspace',
    })
  })

  it('opens a catalog session read-only until an explicit restart', async () => {
    const { backend, hosts, inputs } = createBackend()
    await backend.bootstrap()
    const stopped = await backend.openSession('session-2')

    expect(inputs).toEqual([{
      workspaceId: 'local-workspace', cwd: nativeTestPath('/workspace/project'),
    }])
    expect(hosts).toHaveLength(1)
    expect(stopped).toMatchObject({
      session: { id: 'session-2', status: 'stopped' },
      conversation: [], agents: [],
    })

    const runtime = await backend.restartSession('session-2')
    const detail = await backend.openSession('session-2')

    expect(inputs).toEqual([
      { workspaceId: 'local-workspace', cwd: nativeTestPath('/workspace/project') },
      {
        workspaceId: 'local-workspace', cwd: nativeTestPath('/workspace/project'),
        resumeSessionId: 'session-2',
      },
    ])
    expect(hosts).toHaveLength(2)
    expect(runtime).toMatchObject({ sessionId: 'session-2', state: 'ready' })
    expect(detail).toMatchObject({
      session: { id: 'session-2', status: 'waiting', activeRuntimeId: 'runtime-2' },
      conversation: [expect.objectContaining({ text: 'Answer from session-2' })],
    })
  })

  it('retains the final runtime, reports final host status, and restarts a new generation', async () => {
    const { backend, hosts } = createBackend()
    await backend.bootstrap()
    const events: any[] = []
    backend.onEvent((event) => events.push(event))

    await backend.stopSession('session-1')
    expect(events).toContainEqual({ type: 'host_status', status: 'stopping' })
    expect(events).toContainEqual({ type: 'host_status', status: 'stopped' })
    expect(events).toContainEqual({
      type: 'runtime_updated',
      runtime: expect.objectContaining({ id: 'runtime-1', state: 'stopped', startedAt: timestamp.toISOString() }),
    })
    expect(events).toContainEqual({
      type: 'session_updated',
      session: expect.not.objectContaining({ activeRuntimeId: expect.anything(), attention: expect.anything() }),
    })
    const stoppedSnapshot = await backend.bootstrap()
    expect(stoppedSnapshot.hostStatus).toBe('stopped')
    expect(stoppedSnapshot.runtimes).toEqual([
      expect.objectContaining({ id: 'runtime-1', state: 'stopped' }),
    ])
    const hostCount = hosts.length
    const stoppedDetail = await backend.openSession('session-1')
    expect(stoppedDetail).toMatchObject({
      session: { status: 'stopped' },
      conversation: [expect.objectContaining({ text: 'Answer from session-1' })],
    })
    expect(stoppedDetail.session.activeRuntimeId).toBeUndefined()
    expect(hosts).toHaveLength(hostCount)

    const restarted = await backend.restartSession('session-1')
    expect(restarted).toMatchObject({ id: 'runtime-2', generation: 2, state: 'ready' })
    expect(hosts[1]!.sessionId).toBe('session-1')
    const sessionRuntimes = events
      .filter((event) => event.type === 'runtime_updated' && event.runtime.sessionId === 'session-1')
      .map((event) => event.runtime.id)
    expect(sessionRuntimes.at(-1)).toBe('runtime-2')
    expect((await backend.bootstrap()).runtimes).toEqual([
      expect.objectContaining({ id: 'runtime-2', generation: 2, state: 'ready' }),
    ])
  })

  it('enforces the live Host quota and frees capacity after stop', async () => {
    const { backend } = createBackend({ maxLive: 1 })
    await backend.bootstrap()
    await expect(backend.openSession('session-2')).resolves.toMatchObject({
      session: { status: 'stopped' },
    })
    await expect(backend.restartSession('session-2')).rejects.toThrow('quota exceeded (1)')
    await backend.stopSession('session-1')
    await expect(backend.restartSession('session-2')).resolves.toMatchObject({ sessionId: 'session-2' })
  })

  it('latches a crash instead of overwriting it with cleanup stop statuses', async () => {
    const { backend, hosts, registry } = createBackend()
    await backend.bootstrap()
    const events: any[] = []
    backend.onEvent((event) => events.push(event))
    hosts[0]!.crash()
    await vi.waitFor(() => expect(registry.liveCount).toBe(0))
    expect(events).toContainEqual({ type: 'host_status', status: 'crashed' })
    expect(events).toContainEqual({
      type: 'runtime_updated',
      runtime: expect.objectContaining({ id: 'runtime-1', state: 'crashed' }),
    })
    expect(events).toContainEqual({
      type: 'session_updated',
      session: expect.objectContaining({ id: 'session-1', status: 'failed', attention: 'failure' }),
    })
    expect(events).not.toContainEqual({
      type: 'runtime_updated',
      runtime: expect.objectContaining({ id: 'runtime-1', state: 'stopped' }),
    })
    expect(await backend.bootstrap()).toMatchObject({
      hostStatus: 'crashed', runtimes: [expect.objectContaining({ state: 'crashed' })],
    })
    const hostCount = hosts.length
    await expect(backend.openSession('session-1')).resolves.toMatchObject({
      session: { status: 'failed', attention: 'failure' },
      conversation: [expect.objectContaining({ text: 'Answer from session-1' })],
    })
    expect(hosts).toHaveLength(hostCount)
  })

  it('rejects unknown sessions and shuts down every live Host', async () => {
    const { backend, hosts } = createBackend()
    await backend.bootstrap()
    await backend.restartSession('session-2')
    await expect(backend.openSession('missing')).rejects.toThrow('Unknown')
    await expect(backend.sendMessage('missing', 'message')).rejects.toThrow('Unknown')
    await backend.shutdown()
    expect(hosts.map((host) => host.stop.mock.calls.length)).toEqual([1, 1])
    backend.dispose()
    expect(hosts.map((host) => host.dispose.mock.calls.length)).toEqual([1, 1])
  })
})
