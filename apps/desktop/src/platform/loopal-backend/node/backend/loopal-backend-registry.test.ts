import { join } from 'node:path'
import { describe, expect, it, vi } from 'vitest'
import { createBackendRegistry } from './loopal-backend-registry'
import { FakeRuntimeHost } from '../runtime/session-runtime-registry.test-fixtures'
import { SessionRuntimeRegistry } from '../runtime/session-runtime-registry'

const base = { binaryPath: '/bin/loopal', cwd: '/workspace', parentPid: 7 }

describe('createBackendRegistry', () => {
  it('returns an explicitly injected registry', () => {
    const injected = new SessionRuntimeRegistry({
      maxLive: 1, createHost: () => new FakeRuntimeHost('session'),
    })
    expect(createBackendRegistry({ ...base, runtimeRegistry: injected })).toBe(injected)
  })

  it('uses a custom Host factory and configured quota', async () => {
    const registry = createBackendRegistry({
      ...base,
      maxLiveRuntimes: 1,
      createHost: (input) => new FakeRuntimeHost(input.resumeSessionId ?? 'fresh'),
    })
    await expect(registry.resume({
      workspaceId: 'workspace', cwd: '/workspace', sessionId: 'session',
    })).resolves.toMatchObject({ sessionId: 'session' })
    expect(() => registry.startFresh({ workspaceId: 'workspace', cwd: '/workspace' }))
      .toThrow('quota exceeded (1)')
    await registry.shutdownAll()
  })

  it('constructs the production Host with default quota and forwarded options', async () => {
    const registry = createBackendRegistry({
      ...base,
      env: { LOOPAL_TEST: '1' },
      startupTimeoutMs: 10,
      shutdownTimeoutMs: 10,
      clientName: 'coverage',
      spawnProcess: () => { throw new Error('spawn failed') },
      connectRpc: async () => { throw new Error('unused') },
    })
    await expect(registry.resume({
      workspaceId: 'workspace', cwd: '/workspace', sessionId: 'resume-me',
    }))
      .rejects.toThrow('spawn failed')
    expect(registry.liveCount).toBe(0)
  })

  it('keeps every production Host option optional', async () => {
    const registry = createBackendRegistry({
      binaryPath: join(process.cwd(), `.missing-loopal-${process.pid}`),
      cwd: '/workspace',
    })
    await expect(registry.startFresh({ workspaceId: 'workspace', cwd: '/workspace' }))
      .rejects.toThrow()
    expect(registry.liveCount).toBe(0)
  })

  it('reads the MetaHub startup secret once per Host allocation', async () => {
    const startup = {
      address: '127.0.0.1:9000', hubName: 'desktop-a', token: 'secret',
    }
    const getMetaHubStartup = vi.fn(() => startup)
    const spawnProcess = vi.fn((
      _binary: string, _cwd: string, _pid: number | undefined,
      _env: unknown, _resume: unknown, _metaHub: { hubName: string },
    ) => { throw new Error('captured') })
    const registry = createBackendRegistry({
      ...base, getMetaHubStartup, spawnProcess: spawnProcess as never,
    })
    await expect(registry.startFresh({ workspaceId: 'workspace', cwd: '/workspace' }))
      .rejects.toThrow('captured')
    expect(getMetaHubStartup).toHaveBeenCalledOnce()
    expect(spawnProcess).toHaveBeenCalledWith(
      '/bin/loopal', '/workspace', 7, undefined, undefined,
      expect.objectContaining({
        address: startup.address, token: startup.token,
        hubName: expect.stringMatching(/^desktop-a-.+-g1-/),
      }),
    )
  })

  it('gives auto-joined runtimes distinct generation-safe Hub names', async () => {
    const spawnProcess = vi.fn((
      _binary: string, _cwd: string, _pid: number | undefined,
      _env: unknown, _resume: unknown, _metaHub: { hubName: string },
    ) => { throw new Error('captured') })
    const registry = createBackendRegistry({
      ...base,
      getMetaHubStartup: () => ({
        address: '127.0.0.1:9000', hubName: 'desktop-a', token: 'secret',
      }),
      spawnProcess: spawnProcess as never,
    })
    for (const sessionId of ['session-a', 'session-b']) {
      await expect(registry.resume({ workspaceId: 'workspace', cwd: '/workspace', sessionId }))
        .rejects.toThrow('captured')
    }
    const names = spawnProcess.mock.calls.map((call) => call[5]!.hubName)
    expect(names[0]).toMatch(/^desktop-a-session-a-g1-/)
    expect(names[1]).toMatch(/^desktop-a-session-b-g2-/)
    expect(names[0]).not.toBe(names[1])
  })
})
