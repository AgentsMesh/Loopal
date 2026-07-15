import { Emitter } from '../../../../base/common/event'
import { CancellationToken } from '../../../../base/common/cancellation'
import { LoopalMetaHubService } from './loopal-metahub-service'
import { LoopalMetaHubSettings } from './loopal-metahub-settings'
import { type DesktopHostClient } from '../backend/loopal-backend-types'
import { type SessionRuntimeHandle } from '../runtime/session-runtime-registry'

const target = { sessionId: 'session', runtimeId: 'runtime', generation: 1 }

function fixture() {
  let connected = false
  const requests = vi.fn(async (method: string) => {
    if (method === 'hub/status') return {
      agent_count: 1,
      uplink: connected ? {
        connected: true, hub_name: 'desktop-a', address: '127.0.0.1:9000',
      } : null,
    }
    if (method === 'hub/join_meta') { connected = true; return { connected: true } }
    if (method === 'hub/leave_meta') { connected = false; return { connected: false } }
    if (method === 'meta/list_hubs') return { hubs: [{
      name: 'desktop-a', status: 'Connected', agent_count: 1, capabilities: [],
    }] }
    if (method === 'meta/topology') return { hubs: [{
      hub: 'desktop-a', topology: { agents: [] },
    }] }
    throw new Error(`unexpected ${method}`)
  })
  const statuses = new Emitter<never>()
  const notifications = new Emitter<never>()
  const host: DesktopHostClient = {
    currentStatus: 'ready', onStatus: statuses.event, onNotification: notifications.event,
    request: requests, start: async () => ({ sessionId: 'session', serverVersion: '1', pid: 1 }),
    stop: async () => undefined, dispose: () => undefined,
  }
  const runtime: SessionRuntimeHandle = { ...target, workspaceId: 'workspace', host }
  const settings = new LoopalMetaHubSettings()
  const resync = vi.fn(async () => undefined)
  const service = new LoopalMetaHubService({
    settings, runtime: () => runtime, resync, now: () => new Date('2026-01-01T00:00:00Z'),
  })
  return { service, settings, requests, resync, setConnected: (value: boolean) => { connected = value } }
}

describe('LoopalMetaHubService', () => {
  it('updates secret-backed settings, joins, reconnects, refreshes, and disconnects', async () => {
    const value = fixture()
    await value.service.updateMetaHubSettings({
      address: '127.0.0.1:9000', hubName: 'desktop-a', joinOnStart: true,
      startLocalOnLaunch: false, token: 'secret',
    })
    expect(await value.service.getMetaHubSettings()).toMatchObject({ tokenConfigured: true })
    await expect(value.service.joinMetaHub(target)).resolves.toMatchObject({
      state: 'connected', hubName: 'desktop-a',
    })
    await expect(value.service.getMetaHubStatus(target)).resolves.toMatchObject({
      state: 'connected', hubs: [expect.objectContaining({ name: 'desktop-a' })],
    })
    await value.service.joinMetaHub({
      ...target, address: '127.0.0.1:9000', hubName: 'desktop-a', token: 'replacement',
    })
    expect(value.requests.mock.calls.filter(([method]) => method === 'hub/leave_meta')).toHaveLength(1)
    await expect(value.service.disconnectMetaHub(target)).resolves.toMatchObject({
      state: 'disconnected', hubs: [],
    })
    expect(value.resync).toHaveBeenCalledTimes(3)
  })

  it('rejects incomplete credentials and cancellation before RPC', async () => {
    const value = fixture()
    await expect(value.service.joinMetaHub(target)).rejects.toThrow('required')
    await expect(value.service.getMetaHubSettings(CancellationToken.Cancelled)).rejects
      .toThrow('cancelled')
    await expect(value.service.updateMetaHubSettings({
      address: '', hubName: 'desktop-a', joinOnStart: false,
      startLocalOnLaunch: false,
    }, CancellationToken.Cancelled)).rejects.toThrow('cancelled')
  })

  it('leaves an error-state uplink before replacing it', async () => {
    const value = fixture()
    await value.settings.update({
      address: '127.0.0.1:9000', hubName: 'desktop-a', joinOnStart: false,
      startLocalOnLaunch: false, token: 'secret',
    })
    value.setConnected(true)
    value.requests.mockImplementationOnce(async () => ({
      agent_count: 1,
      uplink: { connected: true, hub_name: 'desktop-a', address: '127.0.0.1:9000' },
    }))
    await value.service.joinMetaHub(target)
    expect(value.requests).toHaveBeenCalledWith('hub/leave_meta', {}, expect.any(AbortSignal))
  })
})
