import {
  JoinMetaHubInputSchema,
  LocalMetaHubStatusSchema,
  MetaHubInfoSchema,
  MetaHubRuntimeStateSchema,
  MetaHubRuntimeTargetSchema,
  MetaHubSettingsSchema,
  MetaHubTopologyAgentSchema,
  StartLocalMetaHubInputSchema,
  UpdateMetaHubSettingsInputSchema,
} from './metahub-contracts'

describe('MetaHub contracts', () => {
  const target = { sessionId: 'session', runtimeId: 'runtime', generation: 1 }

  it('parses settings, runtime targets, join inputs, and managed coordinator defaults', () => {
    expect(MetaHubSettingsSchema.parse({
      address: '', hubName: 'desktop-a', joinOnStart: false,
      startLocalOnLaunch: false, tokenConfigured: false,
    })).toMatchObject({ hubName: 'desktop-a' })
    expect(UpdateMetaHubSettingsInputSchema.parse({
      address: ' 127.0.0.1:9 ', hubName: ' desktop-a ', joinOnStart: true,
      startLocalOnLaunch: false, token: 'secret', clearToken: false,
    })).toMatchObject({ address: '127.0.0.1:9', hubName: 'desktop-a' })
    expect(MetaHubRuntimeTargetSchema.parse(target)).toEqual(target)
    expect(JoinMetaHubInputSchema.parse({ ...target, address: 'meta:9' })).toMatchObject(target)
    expect(StartLocalMetaHubInputSchema.parse({})).toEqual({ bindAddress: '127.0.0.1:0' })
    expect(LocalMetaHubStatusSchema.parse({ state: 'failed', error: 'port busy' }))
      .toMatchObject({ state: 'failed' })
  })

  it('parses Hub inventory, qualified topology, and connected runtime state', () => {
    const hub = MetaHubInfoSchema.parse({
      name: 'hub-a', status: 'degraded', agentCount: 2, capabilities: ['desktop'],
    })
    const agent = MetaHubTopologyAgentSchema.parse({
      id: 'hub-a/main', name: 'main', hub: 'hub-a', hubPath: ['hub-a'],
      parentId: 'outer/root', children: ['hub-a/child'], lifecycle: 'failed',
      model: 'gpt-5', error: 'stopped',
    })
    expect(MetaHubRuntimeStateSchema.parse({
      state: 'connected', address: 'meta:9', hubName: 'hub-a', hubs: [hub],
      topology: [agent], refreshedAt: '2026-01-01T00:00:00.000Z',
    })).toMatchObject({ state: 'connected', hubs: [hub] })
  })

  it('rejects unsafe or incomplete values', () => {
    expect(() => UpdateMetaHubSettingsInputSchema.parse({
      address: 'meta:9', hubName: 'bad/name', joinOnStart: false,
      startLocalOnLaunch: false,
    })).toThrow("cannot contain '/'")
    expect(() => MetaHubRuntimeTargetSchema.parse({ ...target, generation: 0 })).toThrow()
    expect(() => JoinMetaHubInputSchema.parse({ ...target, token: '' })).toThrow()
    expect(() => MetaHubTopologyAgentSchema.parse({
      id: 'x', name: 'x', hub: 'x', hubPath: [], children: [], lifecycle: 'running',
    })).toThrow()
  })
})
