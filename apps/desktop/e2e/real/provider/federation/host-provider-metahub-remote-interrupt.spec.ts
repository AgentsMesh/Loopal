import { expect, test, type Page } from '@playwright/test'
import { closeDesktop, launchDesktop, type DesktopFixture } from '../../../support/electron/electron-fixture'
import { ready, send } from '../../../support/runtime/llm-e2e-helpers'
import {
  startMetaHub, startRemoteHub, stopProcess,
  type MetaHubProcess, type RemoteHubProcess,
} from '../../../support/federation/metahub-data-plane-fixture'

const active = 'REMOTE INTERRUPT STREAM ACTIVE 84F1'
const late = 'REMOTE INTERRUPT LATE OUTPUT 84F1'

test('interrupts a running remote child without late completion', async () => {
  let meta: MetaHubProcess | undefined
  let desktop: DesktopFixture | undefined
  let remote: RemoteHubProcess | undefined
  try {
    meta = await startMetaHub()
    desktop = await launchDesktop('real', 'metahub-remote-interrupt')
    await ready(desktop.page)
    const target = await joinDesktop(desktop, meta)
    remote = await startRemoteHub(desktop, meta)
    await expect.poll(() => cluster(desktop!.page), { timeout: 15_000 }).toEqual({
      state: 'connected', hubs: ['hub-a', 'hub-b'], remoteMain: 'running',
    })

    await send(desktop.page, 'Spawn the remote interrupt child')
    await expect(desktop.page.getByTestId('conversation')).toContainText(
      'Remote cancellable child is running.', { timeout: 15_000 },
    )
    await expect.poll(() => child(desktop!.page), { timeout: 15_000 }).toMatchObject({
      id: 'hub-b/remote-cancel', status: 'running', controllable: true,
    })
    await expect.poll(() => desktop!.llm!.state()).toMatchObject({ inFlight: 1 })

    await desktop.page.evaluate(async ({ root, agentId }) => {
      await window.loopalDesktop.interruptAgent({ ...root, agentId })
    }, { root: target, agentId: 'hub-b/remote-cancel' })
    await expect.poll(() => desktop!.llm!.state(), { timeout: 15_000 }).toMatchObject({
      clientDisconnects: 1, served: 4, remaining: 0, inFlight: 0,
      unmatchedRequests: 0, verified: true,
    })
    await expect(desktop.page.getByTestId('conversation')).toContainText(
      'Root observed remote child interruption.', { timeout: 15_000 },
    )
    await expect.poll(() => child(desktop!.page), { timeout: 15_000 }).toMatchObject({
      id: 'hub-b/remote-cancel', status: 'completed', controllable: false,
    })

    await desktop.page.waitForTimeout(1_500)
    await expect(desktop.page.getByTestId('conversation')).not.toContainText(late)
    const requests = await desktop.llm!.requests()
    expect(requests).toHaveLength(4)
    expect(requests.every((request) => request.matched)).toBe(true)
    expect(requests.filter((request) => request.lastUserText.includes(active))).toHaveLength(1)
    expect(await desktop.llm!.state()).toMatchObject({
      clientDisconnects: 1, served: 4, remaining: 0, inFlight: 0, verified: true,
    })
  } finally {
    remote?.probe.close()
    if (remote) await stopProcess(remote.child)
    if (desktop) await closeDesktop(desktop)
    if (meta) await stopProcess(meta.child)
  }
})

async function joinDesktop(desktop: DesktopFixture, meta: MetaHubProcess) {
  return desktop.page.evaluate(async ({ address, token }) => {
    const api = window.loopalDesktop
    const bootstrap = await api.bootstrap()
    const sessionId = bootstrap.activeSessionId!
    const runtime = bootstrap.runtimes.find((item) => item.sessionId === sessionId)!
    const target = {
      sessionId, runtimeId: runtime.id, generation: runtime.generation,
      agentId: runtime.rootAgent,
    }
    await api.controlAgent({ target, command: { type: 'permission', mode: 'bypass' } })
    await api.updateMetaHubSettings({
      address, token, hubName: 'hub-a', joinOnStart: false, startLocalOnLaunch: false,
    })
    await api.joinMetaHub(target)
    return target
  }, { address: meta.address, token: meta.token })
}

async function cluster(page: Page) {
  const detail = await detailOf(page)
  return {
    state: detail.metaHub?.state,
    hubs: detail.metaHub?.hubs.map((hub) => hub.name).sort() ?? [],
    remoteMain: detail.metaHub?.topology
      .find((agent) => agent.id === 'hub-b/main')?.lifecycle,
  }
}

async function child(page: Page) {
  return (await detailOf(page)).agents.find((agent) => agent.id === 'hub-b/remote-cancel')
}

async function detailOf(page: Page) {
  return page.evaluate(async () => {
    const bootstrap = await window.loopalDesktop.bootstrap()
    return window.loopalDesktop.openSession(bootstrap.activeSessionId!)
  })
}
