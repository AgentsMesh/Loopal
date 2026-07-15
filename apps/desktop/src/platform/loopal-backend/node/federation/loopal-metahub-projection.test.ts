import { Emitter } from '../../../../base/common/event'
import { type DesktopHostClient } from '../backend/loopal-backend-types'
import {
  loadMetaHubState,
  mergeRemoteAgents,
  projectTopology,
} from './loopal-metahub-projection'

function host(request: DesktopHostClient['request']): DesktopHostClient {
  const status = new Emitter<never>()
  const notifications = new Emitter<never>()
  return {
    currentStatus: 'ready',
    onStatus: status.event,
    onNotification: notifications.event,
    request,
    start: async () => ({ sessionId: 's', serverVersion: '1', pid: 1 }),
    stop: async () => undefined,
    dispose: () => { status.dispose(); notifications.dispose() },
  }
}

const status = {
  agent_count: 1,
  uplink: { connected: true, hub_name: 'hub-a', address: '127.0.0.1:9' },
}
const list = {
  hubs: [
    { name: 'hub-b', status: 'Degraded', agent_count: 2, capabilities: [] },
    { name: 'hub-a', status: 'Connected', agent_count: 1, capabilities: ['desktop'] },
  ],
}
const topology = {
  hubs: [{ hub: 'hub-b', topology: { agents: [{
    name: 'child', parent: 'hub-a/main', children: [], lifecycle: 'running' as const,
  }] } }],
}

describe('MetaHub projection', () => {
  it('fails open when an older host does not expose Hub status', async () => {
    const value = await loadMetaHubState(host(async () => {
      throw new Error('method not found')
    }), new Date('2026-01-01T00:00:00Z'))
    expect(value).toEqual({
      state: 'error', hubs: [], topology: [], error: 'method not found',
      refreshedAt: '2026-01-01T00:00:00.000Z',
    })
  })

  it('deduplicates concurrent loads and serves the short-lived cache', async () => {
    let release!: () => void
    const gate = new Promise<void>((resolve) => { release = resolve })
    const request = vi.fn(async (method: string) => {
      if (method === 'hub/status') return status
      await gate
      return method === 'meta/list_hubs' ? list : topology
    })
    const target = host(request)
    const now = new Date('2026-01-01T00:00:00Z')
    const first = loadMetaHubState(target, now)
    const second = loadMetaHubState(target, now)
    await vi.waitFor(() => expect(request).toHaveBeenCalledWith(
      'hub/status', {}, expect.any(AbortSignal),
    ))
    release()
    expect(await first).toEqual(await second)
    expect(request).toHaveBeenCalledTimes(3)
    await loadMetaHubState(target, new Date(now.getTime() + 500))
    expect(request).toHaveBeenCalledTimes(3)
    await loadMetaHubState(target, new Date(now.getTime() + 500), true)
    expect(request).toHaveBeenCalledTimes(6)
  })

  it('bounds remote queries and reports a state error without rejecting', async () => {
    vi.useFakeTimers()
    const target = host(async (method, _params, signal) => {
      if (method === 'hub/status') return status
      return new Promise((_, reject) => {
        signal?.addEventListener('abort', () => reject(new Error('query aborted')))
      })
    })
    const pending = loadMetaHubState(target, new Date(), true)
    await vi.advanceTimersByTimeAsync(2_501)
    await expect(pending).resolves.toMatchObject({ state: 'error', error: 'query aborted' })
    vi.useRealTimers()
  })

  it('projects qualified paths and joins remote parents to the local root', () => {
    const projected = projectTopology(topology)
    expect(projected[0]).toMatchObject({
      id: 'hub-b/child', parentId: 'hub-a/main', hubPath: ['hub-b'],
    })
    const merged = mergeRemoteAgents([
      { id: 'main', name: 'Loopal', status: 'running', children: ['child'] },
      { id: 'child', name: 'child', status: 'running', parentId: 'main', shadow: true },
    ], {
      state: 'connected', hubName: 'hub-a', hubs: [], topology: projected,
      refreshedAt: new Date().toISOString(),
    })
    expect(merged[1]).toMatchObject({
      id: 'hub-b/child', parentId: 'main', qualifiedName: 'hub-b/child', controllable: true,
    })
    expect(merged).toHaveLength(2)
    expect(merged[0]?.children).toEqual([])

    const completed = mergeRemoteAgents([
      { id: 'main', name: 'Loopal', status: 'waiting', children: ['child'] },
      { id: 'child', name: 'child', status: 'completed', parentId: 'main', shadow: true },
    ], {
      state: 'connected', hubName: 'hub-a', hubs: [],
      topology: projected.map((agent) => ({ ...agent, lifecycle: 'finished' as const })),
      refreshedAt: new Date().toISOString(),
    })
    expect(completed.map((agent) => agent.id)).toEqual(['main', 'hub-b/child'])
    expect(completed[1]).toMatchObject({ status: 'completed', qualifiedName: 'hub-b/child' })

    const roots = mergeRemoteAgents([
      { id: 'main', name: 'Loopal', status: 'waiting', children: [] },
    ], {
      state: 'connected', hubName: 'hub-a', hubs: [], topology: [{
        id: 'hub-b/main', name: 'main', hub: 'hub-b', hubPath: ['hub-b'],
        children: [], lifecycle: 'running',
      }], refreshedAt: new Date().toISOString(),
    })
    expect(roots.map((agent) => agent.id)).toEqual(['main', 'hub-b/main'])
  })

  it('projects partial, failed, and error-bearing remote topologies', () => {
    const projected = projectTopology({ hubs: [
      { hub: 'broken', topology: { error: 'unreachable' } },
      { hub: 'remote', topology: { agents: [
        {
          name: 'starting', parent: 'root', children: ['failed'], lifecycle: 'spawning',
          model: 'mock-model', error: null,
        },
        {
          name: 'failed', parent: null, children: [], lifecycle: 'failed',
          model: null, error: 'worker stopped',
        },
      ] } },
    ] })
    expect(projected).toEqual([
      expect.objectContaining({
        id: 'remote/failed', error: 'worker stopped', lifecycle: 'failed',
      }),
      expect.objectContaining({
        id: 'remote/starting', parentId: 'remote/root', model: 'mock-model',
        children: ['remote/failed'], lifecycle: 'spawning',
      }),
    ])

    const merged = mergeRemoteAgents([], {
      state: 'connected', hubName: 'local', hubs: [], topology: projected,
      refreshedAt: '2026-01-01T00:00:00.000Z',
    })
    expect(merged).toEqual([
      expect.objectContaining({ id: 'remote/failed', status: 'failed' }),
      expect.objectContaining({ id: 'remote/starting', status: 'starting' }),
    ])
  })
})
