import { expect, test } from '@playwright/test'
import {
  closeDesktop, launchDesktop, type DesktopFixture,
} from '../../../support/electron/electron-fixture'
import {
  startMetaHub, startRemoteHub, stopProcess,
  type MetaHubProcess, type RemoteHubProcess,
} from '../../../support/federation/metahub-data-plane-fixture'

test('removes a crashed remote Hub and accepts one clean same-name replacement', async () => {
  let meta: MetaHubProcess | undefined
  let desktop: DesktopFixture | undefined
  let remote: RemoteHubProcess | undefined
  try {
    meta = await startMetaHub()
    desktop = await launchDesktop('real')
    const sessionId = await joinDesktop(desktop, meta)
    remote = await startRemoteHub(desktop, meta, 'hub-restart')
    await expect.poll(() => remoteCount(desktop!, sessionId), { timeout: 15_000 }).toBe(1)

    remote.probe.close()
    await stopProcess(remote.child)
    remote = undefined
    await expect.poll(() => remoteCount(desktop!, sessionId), { timeout: 15_000 }).toBe(0)

    remote = await startRemoteHub(desktop, meta, 'hub-restart')
    await expect.poll(() => remoteCount(desktop!, sessionId), { timeout: 15_000 }).toBe(1)
    const state = await desktop.page.evaluate(
      (id) => window.loopalDesktop.openSession(id), sessionId,
    )
    expect(state.metaHub?.hubs.filter((hub) => hub.name === 'hub-restart')).toHaveLength(1)
    expect(state.metaHub?.topology.filter((agent) => agent.id === 'hub-restart/main'))
      .toHaveLength(1)
  } finally {
    remote?.probe.close()
    if (remote) await stopProcess(remote.child)
    if (desktop) await closeDesktop(desktop)
    if (meta) await stopProcess(meta.child)
  }
})

async function joinDesktop(desktop: DesktopFixture, meta: MetaHubProcess): Promise<string> {
  return desktop.page.evaluate(async ({ address, token }) => {
    const api = window.loopalDesktop
    const bootstrap = await api.bootstrap()
    const sessionId = bootstrap.activeSessionId!
    const runtime = bootstrap.runtimes.find((item) => item.sessionId === sessionId)!
    const target = {
      sessionId, runtimeId: runtime.id, generation: runtime.generation,
      agentId: runtime.rootAgent,
    }
    await api.updateMetaHubSettings({
      address, hubName: 'hub-a', token,
      joinOnStart: false, startLocalOnLaunch: false,
    })
    await api.joinMetaHub(target)
    return sessionId
  }, { address: meta.address, token: meta.token })
}

async function remoteCount(desktop: DesktopFixture, sessionId: string): Promise<number> {
  const detail = await desktop.page.evaluate(
    (id) => window.loopalDesktop.openSession(id), sessionId,
  )
  return detail.metaHub?.topology.filter((agent) => agent.id === 'hub-restart/main').length ?? 0
}
