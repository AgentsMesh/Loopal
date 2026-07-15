import { describe, expect, it, vi } from 'vitest'
import {
  createRegistryHarness,
  FakeRuntimeHost,
  workspaceA,
} from './session-runtime-registry.test-fixtures'
import { SessionRuntimeRegistry } from './session-runtime-registry'

describe('SessionRuntimeRegistry edges', () => {
  it('reports pending entries, replays buffered notifications, and exposes membership', async () => {
    const harness = createRegistryHarness()
    const notifications: unknown[] = []
    harness.registry.onNotification((event) => notifications.push(event))
    harness.delayNext()
    const pending = harness.registry.resume({ ...workspaceA, sessionId: 'session-a' })
    expect(harness.registry.listLive()).toEqual([])
    harness.hosts[0]!.notify('agent/event', { buffered: true })
    harness.hosts[0]!.releaseStart()
    const runtime = await pending
    expect(harness.registry.has(runtime.runtimeId)).toBe(true)
    expect(notifications).toEqual([{
      workspaceId: 'workspace-a', sessionId: 'session-a', runtimeId: 'runtime-1', generation: 1,
      method: 'agent/event', params: { buffered: true },
    }])
  })

  it('makes tombstone stop idempotent and rejects an unrestartable ID', async () => {
    const { registry } = createRegistryHarness()
    const runtime = await registry.resume({ ...workspaceA, sessionId: 'session-a' })
    await registry.stop(runtime.runtimeId)
    await expect(registry.stop(runtime.runtimeId)).resolves.toBeUndefined()
    await expect(registry.restart('missing')).rejects.toThrow('cannot be restarted')
  })

  it('rejects duplicate live and tombstoned runtime IDs', async () => {
    const liveHosts: FakeRuntimeHost[] = []
    const live = new SessionRuntimeRegistry({
      maxLive: 2,
      createRuntimeId: () => 'duplicate',
      createHost: () => {
        const host = new FakeRuntimeHost(`session-${liveHosts.length}`)
        liveHosts.push(host)
        return host
      },
    })
    await live.startFresh(workspaceA)
    expect(() => live.startFresh(workspaceA)).toThrow('Duplicate runtime ID')
    await live.shutdownAll()

    const tombstone = new SessionRuntimeRegistry({
      maxLive: 1,
      createRuntimeId: () => 'same',
      createHost: (input) => new FakeRuntimeHost(input.resumeSessionId ?? 'session'),
    })
    const runtime = await tombstone.resume({ ...workspaceA, sessionId: 'session' })
    await tombstone.stop(runtime.runtimeId)
    expect(() => tombstone.startFresh(workspaceA)).toThrow('Duplicate runtime ID')
  })

  it('rejects conflicting fresh session ownership', async () => {
    const registry = new SessionRuntimeRegistry({
      maxLive: 2,
      createRuntimeId: (() => { let value = 0; return () => `runtime-${++value}` })(),
      createHost: () => new FakeRuntimeHost('same-session'),
    })
    await registry.startFresh(workspaceA)
    await expect(registry.startFresh(workspaceA)).rejects.toThrow('already has a live runtime')
    await registry.shutdownAll()
  })

  it('validates text and tombstone limits', () => {
    expect(() => new SessionRuntimeRegistry({
      maxLive: 1, maxTombstones: 0, createHost: () => new FakeRuntimeHost('session'),
    })).toThrow('maxTombstones')
    const registry = new SessionRuntimeRegistry({
      maxLive: 1, createHost: () => new FakeRuntimeHost('session'),
    })
    expect(() => registry.resume({ ...workspaceA, sessionId: ' ' })).toThrow('sessionId')
    expect(() => registry.startFresh({ ...workspaceA, workspaceId: ' ' })).toThrow('workspaceId')
    expect(() => registry.startFresh({ ...workspaceA, cwd: ' ' })).toThrow('cwd')
  })

  it('swallows asynchronous cleanup failures when disposed or crash-retired', async () => {
    const first = createRegistryHarness()
    await first.registry.resume({ ...workspaceA, sessionId: 'session-a' })
    first.hosts[0]!.stopError = new Error('dispose stop failed')
    first.registry.dispose()
    await vi.waitFor(() => expect(first.registry.liveCount).toBe(0))

    const second = createRegistryHarness()
    await second.registry.resume({ ...workspaceA, sessionId: 'session-b' })
    second.hosts[0]!.stopError = new Error('crash stop failed')
    second.hosts[0]!.crash()
    await vi.waitFor(() => expect(second.registry.liveCount).toBe(0))
  })
})
