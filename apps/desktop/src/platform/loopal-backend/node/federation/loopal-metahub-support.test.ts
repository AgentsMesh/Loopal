import { CancellationToken } from '../../../../base/common/cancellation'
import { bindFakeMetaHub } from '../fake/fake-metahub'
import { bindLocalMetaHub, startLocalOnLaunch } from './loopal-local-metahub'
import { bindMetaHub } from './loopal-metahub-bind'
import { type LoopalMetaHubCoordinator } from './loopal-metahub-coordinator'
import { LoopalMetaHubSettings } from './loopal-metahub-settings'
import { LoopalMetaHubRuntime } from './loopal-metahub-runtime'
import { bindUnavailableMetaHub } from '../unavailable/unavailable-metahub'

const target = { sessionId: 'session', runtimeId: 'runtime', generation: 1 }

describe('MetaHub support bindings', () => {
  it('runs local coordinator operations and fail-open autostart', async () => {
    let status = { state: 'stopped' as const }
    let ownedAddress: string | undefined
    const coordinator = {
      get status() { return status },
      get ownedAddress() { return ownedAddress },
      start: vi.fn(async () => {
        ownedAddress = '127.0.0.1:9'
        status = { state: 'running' as const, address: ownedAddress } as never
        return { address: '127.0.0.1:9', token: 'managed-secret' }
      }),
      stop: vi.fn(async () => {
        status = { state: 'stopped' }
        ownedAddress = undefined
      }),
    } as unknown as LoopalMetaHubCoordinator
    const settings = new LoopalMetaHubSettings()
    const operations = bindLocalMetaHub(coordinator, settings)
    expect(await operations.getLocalMetaHubStatus()).toEqual({ state: 'stopped' })
    await expect(operations.startLocalMetaHub({ bindAddress: '127.0.0.1:0' }))
      .resolves.toEqual({ state: 'running', address: '127.0.0.1:9' })
    expect(settings.credentials).toMatchObject({ token: 'managed-secret' })
    await expect(operations.stopLocalMetaHub()).resolves.toEqual({ state: 'stopped' })
    expect(settings.credentials).toBeUndefined()
    await expect(operations.getLocalMetaHubStatus(CancellationToken.Cancelled)).rejects
      .toThrow('cancelled')

    await settings.update({
      address: '', hubName: 'local', joinOnStart: false,
      startLocalOnLaunch: true,
    })
    await startLocalOnLaunch(coordinator, settings)
    expect(coordinator.start).toHaveBeenCalled()
    const failingValue = { start: vi.fn(async () => { throw new Error('no port') }) }
    const failing = failingValue as unknown as LoopalMetaHubCoordinator
    await expect(startLocalOnLaunch(failing, settings)).resolves.toBeUndefined()
    await settings.update({
      address: '', hubName: 'local', joinOnStart: false,
      startLocalOnLaunch: false,
    })
    await startLocalOnLaunch(coordinator, settings)
  })

  it('provides complete deterministic fake operations', async () => {
    const publish = vi.fn()
    const operations = bindFakeMetaHub(
      (value) => value.runtimeId === 'runtime', publish,
    )
    expect(await operations.getMetaHubSettings()).toMatchObject({ tokenConfigured: false })
    await operations.updateMetaHubSettings({
      address: 'meta:9', hubName: 'fake-a', joinOnStart: true,
      startLocalOnLaunch: false, token: 'secret',
    })
    await expect(operations.joinMetaHub(target)).resolves.toMatchObject({ state: 'connected' })
    expect(publish).toHaveBeenLastCalledWith(target, expect.objectContaining({ state: 'connected' }))
    await expect(operations.getMetaHubStatus(target)).resolves.toMatchObject({
      topology: [expect.objectContaining({ id: 'fake-a/main' })],
    })
    await expect(operations.startLocalMetaHub({ bindAddress: '127.0.0.1:0' }))
      .resolves.toMatchObject({ state: 'running' })
    await expect(operations.stopLocalMetaHub()).resolves.toEqual({ state: 'stopped' })
    expect(publish).toHaveBeenLastCalledWith(target, expect.objectContaining({ state: 'disconnected' }))
    await expect(operations.joinMetaHub(target)).rejects.toThrow('token')
    await operations.updateMetaHubSettings({
      address: 'meta:9', hubName: 'fake-a', joinOnStart: false,
      startLocalOnLaunch: false, token: 'secret',
    })
    await operations.joinMetaHub(target)
    await expect(operations.disconnectMetaHub(target)).resolves.toMatchObject({
      state: 'disconnected',
    })
    await expect(operations.getMetaHubStatus({ ...target, runtimeId: 'gone' })).rejects
      .toThrow('gone')
    await operations.updateMetaHubSettings({
      address: '', hubName: 'fake-a', joinOnStart: false,
      startLocalOnLaunch: false, clearToken: true,
    })
    await expect(operations.joinMetaHub(target)).rejects.toThrow('token')
    await bindFakeMetaHub(() => true).getMetaHubSettings()
  })

  it('clears a crashed managed credential without touching replacement external settings', async () => {
    const settings = new LoopalMetaHubSettings()
    await settings.update({
      address: '127.0.0.1:4567', hubName: 'local', joinOnStart: true,
      startLocalOnLaunch: false, token: 'local-secret',
    })
    const coordinator = {
      status: { state: 'failed', error: 'crashed' },
      ownedAddress: '127.0.0.1:4567',
      stop: vi.fn(async () => undefined),
    } as unknown as LoopalMetaHubCoordinator
    const operations = bindLocalMetaHub(coordinator, settings)
    await operations.stopLocalMetaHub()
    expect(settings.publicValue).toMatchObject({
      address: '', joinOnStart: false, tokenConfigured: false,
    })

    await settings.update({
      address: 'meta.example:9000', hubName: 'external', joinOnStart: true,
      startLocalOnLaunch: false, token: 'external-secret',
    })
    await operations.stopLocalMetaHub()
    expect(settings.credentials?.address).toBe('meta.example:9000')
  })

  it('exposes safe unavailable defaults and rejects mutations', async () => {
    const operations = bindUnavailableMetaHub('sidecar missing')
    expect(await operations.getMetaHubSettings()).toMatchObject({ tokenConfigured: false })
    expect(await operations.getLocalMetaHubStatus()).toEqual({ state: 'stopped' })
    await expect(operations.joinMetaHub(target)).rejects.toThrow('sidecar missing')
    await expect(operations.updateMetaHubSettings({
      address: '', hubName: 'x', joinOnStart: false, startLocalOnLaunch: false,
    })).rejects.toThrow('sidecar missing')
    await expect(operations.getMetaHubStatus(target)).rejects.toThrow('sidecar missing')
    await expect(operations.disconnectMetaHub(target)).rejects.toThrow('sidecar missing')
    await expect(operations.startLocalMetaHub({ bindAddress: '127.0.0.1:0' })).rejects
      .toThrow('sidecar missing')
    await expect(operations.stopLocalMetaHub()).rejects.toThrow('sidecar missing')
  })

  it('binds exact runtime generation and requests a live resync', async () => {
    const settings = new LoopalMetaHubSettings()
    const host = { currentStatus: 'ready', request: vi.fn(async () => ({
      agent_count: 1, uplink: null,
    })) }
    const resync = vi.fn(async () => undefined)
    const directory = {
      runtimeForSession: () => ({ ...target, workspaceId: 'workspace', host }),
      liveSession: () => ({ resync }),
    }
    const operations = bindMetaHub(settings, directory as never, () => new Date())
    await expect(operations.getMetaHubStatus(target)).resolves.toMatchObject({
      state: 'disconnected',
    })
    await expect(operations.getMetaHubStatus({ ...target, generation: 2 })).rejects
      .toMatchObject({ code: 'RUNTIME_GONE' })

    const composition = new LoopalMetaHubRuntime({
      binaryPath: '/missing-loopal', parentPid: process.pid,
    })
    await composition.load()
    expect(composition.startup).toBeUndefined()
    expect(composition.operations(directory as never, () => new Date()))
      .toHaveProperty('joinMetaHub')
    await composition.flush()
    await composition.stop()
    composition.dispose()
  })
})
