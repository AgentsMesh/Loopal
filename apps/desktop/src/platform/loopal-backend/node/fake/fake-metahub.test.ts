import { FakeDesktopBackend } from './fake-backend'
import { bindFakeMetaHub } from './fake-metahub'

const first = { sessionId: 'session-a', runtimeId: 'runtime-a', generation: 1 }
const second = { sessionId: 'session-b', runtimeId: 'runtime-b', generation: 2 }
const restarted = { sessionId: 'session-a', runtimeId: 'runtime-a-new', generation: 2 }

describe('fake MetaHub membership', () => {
  it('tracks exact runtime targets independently and publishes every disconnect on stop', async () => {
    const live = new Set([key(first), key(second)])
    const publish = vi.fn()
    const operations = bindFakeMetaHub((target) => live.has(key(target)), publish)
    await operations.updateMetaHubSettings({
      address: 'meta:9', hubName: 'fake', joinOnStart: false,
      startLocalOnLaunch: false, token: 'secret',
    })

    await operations.joinMetaHub({ ...first, hubName: 'fake-a' })
    await operations.joinMetaHub({ ...second, hubName: 'fake-b' })
    await expect(operations.getMetaHubStatus(first)).resolves.toMatchObject({
      state: 'connected', hubName: 'fake-a',
      hubs: [expect.objectContaining({ name: 'fake-a' }), expect.objectContaining({ name: 'fake-b' })],
      topology: [expect.objectContaining({ id: 'fake-a/main' }),
        expect.objectContaining({ id: 'fake-b/main' })],
    })
    await operations.disconnectMetaHub(first)
    await expect(operations.getMetaHubStatus(first)).resolves.toMatchObject({
      state: 'disconnected',
    })
    await expect(operations.getMetaHubStatus(second)).resolves.toMatchObject({
      state: 'connected',
    })

    await operations.joinMetaHub({ ...first, hubName: 'fake-a' })
    publish.mockClear()
    await operations.stopLocalMetaHub()
    expect(publish.mock.calls).toEqual(expect.arrayContaining([
      [first, expect.objectContaining({ state: 'disconnected' })],
      [second, expect.objectContaining({ state: 'disconnected' })],
    ]))
    expect(publish).toHaveBeenCalledTimes(2)
  })

  it('uses the fake backend active ready generation as its authority', async () => {
    const backend = new FakeDesktopBackend()
    const runtimes = (await backend.bootstrap()).runtimes
    const desktop = runtimeTarget(runtimes.find(({ sessionId }) => sessionId === 'session-desktop')!)
    const protocol = runtimeTarget(runtimes.find(({ sessionId }) => sessionId === 'session-protocol')!)
    await backend.startLocalMetaHub({ bindAddress: '127.0.0.1:0' })

    await backend.joinMetaHub({ ...desktop, hubName: 'fake-desktop' })
    await backend.joinMetaHub({ ...protocol, hubName: 'fake-protocol' })
    await backend.disconnectMetaHub(desktop)
    await expect(backend.getMetaHubStatus(desktop)).resolves.toMatchObject({ state: 'disconnected' })
    await expect(backend.getMetaHubStatus(protocol)).resolves.toMatchObject({ state: 'connected' })

    await expect(backend.getMetaHubStatus({ ...desktop, generation: 99 })).rejects
      .toThrow('runtime is gone')
    const restarted = await backend.restartSession(desktop.sessionId)
    await expect(backend.getMetaHubStatus(desktop)).rejects.toThrow('runtime is gone')
    await expect(backend.getMetaHubStatus(runtimeTarget(restarted))).resolves
      .toMatchObject({ state: 'disconnected' })
    await backend.stopSession(protocol.sessionId)
    await expect(backend.getMetaHubStatus(protocol)).rejects.toThrow('runtime is gone')
    backend.dispose()
  })

  it('prunes stopped generations before computing state or publishing', async () => {
    const live = new Set([key(first), key(second)])
    const publish = vi.fn()
    const operations = bindFakeMetaHub((target) => live.has(key(target)), publish)
    await operations.updateMetaHubSettings({
      address: 'meta:9', hubName: 'fake', joinOnStart: false,
      startLocalOnLaunch: false, token: 'secret',
    })
    await operations.joinMetaHub({ ...first, hubName: 'fake-old' })
    await operations.joinMetaHub({ ...second, hubName: 'fake-peer' })

    live.delete(key(first))
    live.add(key(restarted))
    publish.mockClear()
    await expect(operations.joinMetaHub({ ...restarted, hubName: 'fake-new' })).resolves
      .toMatchObject({
        hubs: [expect.objectContaining({ name: 'fake-peer' }),
          expect.objectContaining({ name: 'fake-new' })],
      })
    expect(publishedKeys(publish)).toEqual(expect.arrayContaining([
      key(second), key(restarted),
    ]))
    expect(publishedKeys(publish)).not.toContain(key(first))

    publish.mockClear()
    await operations.stopLocalMetaHub()
    expect(publishedKeys(publish)).not.toContain(key(first))
  })
})

function key(target: { sessionId: string; runtimeId: string; generation: number }): string {
  return JSON.stringify([target.sessionId, target.runtimeId, target.generation])
}

function runtimeTarget(runtime: { id: string; sessionId: string; generation: number }) {
  return { sessionId: runtime.sessionId, runtimeId: runtime.id, generation: runtime.generation }
}

function publishedKeys(publish: ReturnType<typeof vi.fn>): string[] {
  return publish.mock.calls.map(([target]) => key(target))
}
