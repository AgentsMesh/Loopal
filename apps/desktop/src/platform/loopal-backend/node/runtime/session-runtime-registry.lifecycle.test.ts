import { describe, expect, it, vi } from 'vitest'
import {
  createRegistryHarness,
  workspaceA,
  workspaceB,
} from './session-runtime-registry.test-fixtures'

describe('SessionRuntimeRegistry lifecycle', () => {
  it('keeps two Hosts live and scopes their status and notifications', async () => {
    const { registry, hosts } = createRegistryHarness()
    const statuses: unknown[] = []
    const notifications: unknown[] = []
    registry.onStatus((event) => statuses.push(event))
    registry.onNotification((event) => notifications.push(event))

    const [a, b] = await Promise.all([
      registry.startFresh(workspaceA),
      registry.resume({ ...workspaceB, sessionId: 'session-b' }),
    ])

    expect(registry.liveCount).toBe(2)
    expect(registry.listLive()).toEqual([a, b])
    expect([a.runtimeId, b.runtimeId]).toEqual(['runtime-1', 'runtime-2'])
    expect([a.generation, b.generation]).toEqual([1, 2])
    hosts[0]!.notify('agent/event', { value: 'a' })
    hosts[1]!.notify('agent/event', { value: 'b' })
    expect(notifications).toEqual([
      { ...scope(a), method: 'agent/event', params: { value: 'a' } },
      { ...scope(b), method: 'agent/event', params: { value: 'b' } },
    ])
    expect(statuses).toContainEqual({ ...scope(a), status: 'ready' })
    expect(statuses).toContainEqual({ ...scope(b), status: 'ready' })
  })

  it('deduplicates concurrent resume before the Host is ready', async () => {
    const harness = createRegistryHarness()
    harness.delayNext()
    const input = { ...workspaceA, sessionId: 'session-a' }
    const first = harness.registry.resume(input)
    const second = harness.registry.resume(input)

    expect(harness.hosts).toHaveLength(1)
    expect(harness.registry.liveCount).toBe(1)
    harness.hosts[0]!.releaseStart()
    await expect(first).resolves.toBe(await second)
    expect(harness.hosts[0]!.startCalls).toBe(1)
  })

  it('activates an old-Host READY fallback once before flushing fresh status', async () => {
    const { registry, hosts } = createRegistryHarness()
    const activated = vi.fn(async () => undefined)
    const visibility: boolean[] = []
    registry.onStatus(() => visibility.push(activated.mock.calls.length === 1))

    await registry.startFresh(workspaceA, activated)

    expect(activated).toHaveBeenCalledOnce()
    expect(hosts[0]!.activationProvided).toBe(true)
    expect(visibility.length).toBeGreaterThan(0)
    expect(visibility.every(Boolean)).toBe(true)
  })

  it('does not install a creation activation callback when resuming', async () => {
    const { registry, hosts } = createRegistryHarness()
    await registry.resume({ ...workspaceA, sessionId: 'session-a' })
    expect(hosts[0]!.activationProvided).toBe(false)
  })

  it('stops A without touching B', async () => {
    const { registry, hosts } = createRegistryHarness()
    const a = await registry.resume({ ...workspaceA, sessionId: 'session-a' })
    const b = await registry.resume({ ...workspaceB, sessionId: 'session-b' })

    await registry.stop(a.runtimeId)

    expect(registry.liveCount).toBe(1)
    expect(registry.get(a.runtimeId)).toBeUndefined()
    expect(registry.getBySession('session-b')).toBe(b)
    expect(hosts[0]).toMatchObject({ stopCalls: 1, disposeCalls: 1 })
    expect(hosts[1]).toMatchObject({ stopCalls: 0, disposeCalls: 0 })
  })

  it('retires a crashed A without touching B', async () => {
    const { registry, hosts } = createRegistryHarness()
    const a = await registry.resume({ ...workspaceA, sessionId: 'session-a' })
    const b = await registry.resume({ ...workspaceB, sessionId: 'session-b' })

    hosts[0]!.crash()
    await vi.waitFor(() => expect(registry.get(a.runtimeId)).toBeUndefined())

    expect(registry.liveCount).toBe(1)
    expect(registry.getBySession('session-b')).toBe(b)
    expect(hosts[0]).toMatchObject({ stopCalls: 1, disposeCalls: 1 })
    expect(hosts[1]).toMatchObject({ stopCalls: 0, disposeCalls: 0 })
  })

  it('waits for crash retirement before resuming the same session', async () => {
    const harness = createRegistryHarness()
    const old = await harness.registry.resume({ ...workspaceA, sessionId: 'session-a' })
    harness.hosts[0]!.delayStop()
    harness.hosts[0]!.crash()

    const resumed = harness.registry.resume({ ...workspaceA, sessionId: 'session-a' })
    expect(harness.hosts).toHaveLength(1)
    harness.hosts[0]!.releaseStop()

    await expect(resumed).resolves.toMatchObject({
      sessionId: 'session-a', runtimeId: 'runtime-2', generation: 2,
    })
    expect(await resumed).not.toBe(old)
    expect(harness.hosts).toHaveLength(2)
  })
})

function scope(value: {
  workspaceId: string
  sessionId: string
  runtimeId: string
  generation: number
}) {
  return {
    workspaceId: value.workspaceId,
    sessionId: value.sessionId,
    runtimeId: value.runtimeId,
    generation: value.generation,
  }
}
