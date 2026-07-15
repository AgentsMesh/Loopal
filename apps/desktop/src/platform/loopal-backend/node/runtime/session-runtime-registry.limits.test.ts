import { describe, expect, it } from 'vitest'
import { SessionRuntimeRegistry } from './session-runtime-registry'
import {
  createRegistryHarness,
  FakeRuntimeHost,
  workspaceA,
  workspaceB,
} from './session-runtime-registry.test-fixtures'

describe('SessionRuntimeRegistry limits and recovery', () => {
  it('restarts with a new runtime ID and globally increasing generation', async () => {
    const { registry, hosts, inputs } = createRegistryHarness()
    const a = await registry.resume({ ...workspaceA, sessionId: 'session-a' })
    const b = await registry.resume({ ...workspaceB, sessionId: 'session-b' })
    const restarted = await registry.restart(a.runtimeId)

    expect([a.generation, b.generation, restarted.generation]).toEqual([1, 2, 3])
    expect(restarted).toMatchObject({
      runtimeId: 'runtime-3', sessionId: 'session-a', workspaceId: 'workspace-a',
    })
    expect(hosts[0]).toMatchObject({ stopCalls: 1, disposeCalls: 1 })
    expect(inputs[2]).toEqual({
      workspaceId: 'workspace-a', cwd: '/workspace/a', resumeSessionId: 'session-a',
    })
  })

  it('enforces quota atomically and frees it after stop', async () => {
    const { registry } = createRegistryHarness({ maxLive: 1 })
    const a = await registry.resume({ ...workspaceA, sessionId: 'session-a' })
    expect(() => registry.resume({ ...workspaceB, sessionId: 'session-b' }))
      .toThrow('quota exceeded (1)')

    await registry.stop(a.runtimeId)
    await expect(registry.resume({ ...workspaceB, sessionId: 'session-b' }))
      .resolves.toMatchObject({ sessionId: 'session-b' })
  })

  it('bounds tombstones while retaining recent restart data', async () => {
    const { registry } = createRegistryHarness({ maxTombstones: 1 })
    const a = await registry.resume({ ...workspaceA, sessionId: 'session-a' })
    await registry.stop(a.runtimeId)
    const b = await registry.resume({ ...workspaceB, sessionId: 'session-b' })
    await registry.stop(b.runtimeId)

    await expect(registry.stop(a.runtimeId)).rejects.toThrow('Unknown session runtime')
    await expect(registry.restart(b.runtimeId)).resolves.toMatchObject({
      sessionId: 'session-b', generation: 3,
    })
  })

  it('stops every live Host once even when one stop fails', async () => {
    const { registry, hosts } = createRegistryHarness()
    await registry.resume({ ...workspaceA, sessionId: 'session-a' })
    await registry.resume({ ...workspaceB, sessionId: 'session-b' })
    hosts[0]!.stopError = new Error('stop-a failed')

    await expect(registry.shutdownAll()).rejects.toThrow('Failed to stop session runtimes')
    await expect(registry.shutdownAll()).rejects.toThrow('Failed to stop session runtimes')
    expect(hosts.map((host) => host.stopCalls)).toEqual([1, 1])
    expect(hosts.map((host) => host.disposeCalls)).toEqual([1, 1])
    expect(registry.liveCount).toBe(0)
    expect(() => registry.startFresh(workspaceA)).toThrow('shut down')
  })

  it('turns pre-ready notification overflow into a scoped resync', async () => {
    const harness = createRegistryHarness()
    const events: unknown[] = []
    harness.registry.onNotification((event) => events.push(event))
    harness.delayNext()
    const starting = harness.registry.startFresh(workspaceA)
    for (let index = 0; index < 80; index += 1) {
      harness.hosts[0]!.notify('agent/event', { index })
    }
    harness.hosts[0]!.releaseStart()
    const runtime = await starting

    expect(events).toEqual([{
      workspaceId: runtime.workspaceId,
      sessionId: runtime.sessionId,
      runtimeId: runtime.runtimeId,
      generation: runtime.generation,
      method: 'view/resync_required',
      params: { reason: 'pre_ready_buffer_overflow' },
    }])
  })

  it('rejects invalid construction and a mismatched resume result', async () => {
    expect(() => new SessionRuntimeRegistry({ maxLive: 0, createHost: () => undefined as never }))
      .toThrow('positive integer')
    const registry = new SessionRuntimeRegistry({
      maxLive: 1,
      createHost: () => new FakeRuntimeHost('different-session'),
    })
    await expect(registry.resume({ ...workspaceA, sessionId: 'expected-session' }))
      .rejects.toThrow('expected expected-session')
    expect(registry.liveCount).toBe(0)
  })
})
