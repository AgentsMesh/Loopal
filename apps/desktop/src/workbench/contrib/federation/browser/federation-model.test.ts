import { sessionOne, sessionTwo, updatedAt } from '../../../../../test/support/workbench/api-stub'
import { federationHubName } from '../../../../shared/contracts/metahub-identity'
import { type MetaHubRuntimeState } from '../../../../shared/contracts'
import { aggregateFederation, federationTargets } from './federation-model'

const runtimes = [
  { id: 'runtime-1', sessionId: sessionOne.id, workspaceId: 'workspace', generation: 1,
    state: 'ready' as const, rootAgent: 'main' },
  { id: 'runtime-2', sessionId: sessionTwo.id, workspaceId: 'workspace', generation: 2,
    state: 'ready' as const, rootAgent: 'main' },
]

describe('federation model', () => {
  it('resolves authoritative targets and derives unique bounded Hub names', () => {
    const targets = federationTargets([sessionOne, sessionTwo], runtimes)
    expect(targets[sessionOne.id]).toEqual({
      sessionId: sessionOne.id, runtimeId: 'runtime-1', generation: 1,
    })
    const first = federationHubName('x'.repeat(128), targets[sessionOne.id]!)
    const second = federationHubName('x'.repeat(128), targets[sessionTwo.id]!)
    expect(first).not.toBe(second)
    expect(first.length).toBeLessThanOrEqual(128)
    expect(first).not.toContain('/')
  })

  it('aggregates memberships without duplicating a shared topology', () => {
    const targets = federationTargets([sessionOne, sessionTwo], runtimes)
    const state = (hubName: string) => ({
      state: 'connected' as const, address: '127.0.0.1:9', hubName,
      hubs: [{ name: 'shared', status: 'connected' as const, agentCount: 2,
        capabilities: ['desktop'] }],
      topology: [{ id: 'shared/main', name: 'main', hub: 'shared', hubPath: ['shared'],
        children: [], lifecycle: 'running' as const }],
      refreshedAt: updatedAt,
    })
    const snapshot = aggregateFederation(
      { state: 'running', address: '127.0.0.1:9' }, targets,
      Object.values(targets).map((target) => ({ target, state: state(target.sessionId) })),
    )
    expect(snapshot.network).toMatchObject({ state: 'connected' })
    expect(snapshot.network.hubs).toHaveLength(1)
    expect(snapshot.network.topology).toHaveLength(1)
    expect(snapshot.memberships).toEqual({
      [sessionOne.id]: 'connected', [sessionTwo.id]: 'connected',
    })
  })

  it('keeps healthy topology when another membership reports an error', () => {
    const targets = federationTargets([sessionOne, sessionTwo], runtimes)
    const [first, second] = Object.values(targets)
    const address = '127.0.0.1:9'
    const snapshot = aggregateFederation({ state: 'running', address }, targets, [{
      target: first!, state: {
        state: 'connected', address, hubs: [], topology: [], refreshedAt: updatedAt,
      },
    }, {
      target: second!, state: {
        state: 'error', hubs: [], topology: [], error: 'unavailable', refreshedAt: updatedAt,
      },
    }])
    expect(snapshot.network.state).toBe('connected')
    expect(snapshot.network.error).toBe('unavailable')
    expect(snapshot.memberships[sessionTwo.id]).toBe('error')
  })

  it('excludes a runtime connected to a different Federation address', () => {
    const targets = federationTargets([sessionOne, sessionTwo], runtimes)
    const [first, second] = Object.values(targets)
    const connected = (address: string, hub: string): MetaHubRuntimeState => ({
      state: 'connected', address, hubName: hub,
      hubs: [{ name: hub, status: 'connected', agentCount: 1, capabilities: [] }],
      topology: [{ id: `${hub}/main`, name: 'main', hub, hubPath: [hub], children: [],
        lifecycle: 'running' }],
      refreshedAt: updatedAt,
    })
    const snapshot = aggregateFederation(
      { state: 'running', address: 'local:9000' }, targets,
      [{ target: first!, state: connected('local:9000', 'local') },
        { target: second!, state: connected('external:9000', 'external') }],
      'local:9000',
    )
    expect(snapshot.memberships).toEqual({
      [sessionOne.id]: 'connected', [sessionTwo.id]: 'external',
    })
    expect(snapshot.connections).toHaveLength(1)
    expect(snapshot.network.hubs.map(({ name }) => name)).toEqual(['local'])
    expect(snapshot.network.topology.map(({ hub }) => hub)).toEqual(['local'])
  })

  it('preserves colliding agent ids by owner and leaves remote-only nodes unowned', () => {
    const targets = federationTargets([sessionOne, sessionTwo], runtimes)
    const [first, second] = Object.values(targets)
    const topology = (hub: string) => [
      { id: 'main', name: 'main', hub, hubPath: [hub], children: [],
        lifecycle: 'running' as const },
      { id: 'remote/main', name: 'main', hub: 'remote', hubPath: ['remote'], children: [],
        lifecycle: 'running' as const },
    ]
    const address = '127.0.0.1:9'
    const snapshot = aggregateFederation({ state: 'running', address }, targets, [{
      target: first!, state: { state: 'connected', address, hubName: 'one',
        hubs: [], topology: topology('one'), refreshedAt: updatedAt },
    }, {
      target: second!, state: { state: 'connected', address, hubName: 'two',
        hubs: [], topology: topology('two'), refreshedAt: updatedAt },
    }])
    expect(snapshot.topology.map(({ sessionId, agent }) => [sessionId, agent.id])).toEqual([
      [sessionOne.id, 'main'], [undefined, 'remote/main'], [sessionTwo.id, 'main'],
    ])
    expect(snapshot.network.topology).toHaveLength(3)
  })
})
