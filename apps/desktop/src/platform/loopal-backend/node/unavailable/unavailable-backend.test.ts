import { describe, expect, it } from 'vitest'
import { CancellationToken, CancellationTokenSource } from '../../../../base/common/cancellation'
import { UnavailableDesktopBackend } from './unavailable-backend'

describe('UnavailableDesktopBackend', () => {
  it('reports an empty v2 catalog and rejects session operations', async () => {
    const backend = new UnavailableDesktopBackend('host missing')
    await expect(backend.bootstrap()).resolves.toEqual({
      protocolVersion: 2,
      hostStatus: 'stopped',
      workspaces: [],
      sessions: [],
      runtimes: [],
    })
    await expect(backend.openSession('session')).rejects.toThrow('host missing')
    await expect(backend.createSession({
      authorizationId: '5d0c638c-d44c-4f47-818b-62e6b599e31c', launchMode: 'directory',
    })).rejects.toThrow('host missing')
    await expect(backend.stopSession('session')).rejects.toThrow('host missing')
    await expect(backend.restartSession('session')).rejects.toThrow('host missing')
    await expect(backend.sendMessage('session', 'hello')).rejects.toThrow('host missing')
    const target = {
      sessionId: 'session', runtimeId: 'runtime', generation: 1, agentId: 'main',
    }
    await expect(backend.interruptAgent(target)).rejects.toThrow('host missing')
    await expect(backend.controlAgent({
      target, command: { type: 'clear' },
    })).rejects.toThrow('host missing')
    await expect(backend.readFile({ workspaceId: 'w', path: 'a.ts' }, CancellationToken.None))
      .rejects.toThrow('host missing')
    await expect(backend.gitStage({ workspaceId: 'w', path: 'a.ts' }, CancellationToken.None))
      .rejects.toThrow('host missing')
    backend.dispose()
  })

  it('honors cancellation for every operation', async () => {
    const backend = new UnavailableDesktopBackend('host missing')
    const source = new CancellationTokenSource()
    source.cancel()
    await expect(backend.bootstrap(source.token)).rejects.toThrow('cancelled')
    await expect(backend.openSession('session', source.token)).rejects.toThrow('cancelled')
    await expect(backend.sendMessage('session', 'hello', source.token)).rejects.toThrow('cancelled')
    const target = {
      sessionId: 'session', runtimeId: 'runtime', generation: 1, agentId: 'main',
    }
    await expect(backend.interruptAgent(target, source.token)).rejects.toThrow('cancelled')
    await expect(backend.controlAgent({
      target, command: { type: 'clear' },
    }, source.token)).rejects.toThrow('cancelled')
    await expect(backend.gitStatus('w', source.token)).rejects.toThrow('cancelled')
    backend.dispose()
  })
})
