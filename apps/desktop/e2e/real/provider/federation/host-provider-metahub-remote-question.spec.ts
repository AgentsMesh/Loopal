import { expect, test, type Page } from '@playwright/test'
import { closeDesktop, launchDesktop, type DesktopFixture } from '../../../support/electron/electron-fixture'
import { ready, send } from '../../../support/runtime/llm-e2e-helpers'
import {
  startMetaHub, startRemoteHub, stopProcess,
  type MetaHubProcess, type RemoteHubProcess,
} from '../../../support/federation/metahub-data-plane-fixture'

test('returns a Desktop answer to AskUser on a remote child', async () => {
  let meta: MetaHubProcess | undefined
  let desktop: DesktopFixture | undefined
  let remote: RemoteHubProcess | undefined
  try {
    meta = await startMetaHub()
    desktop = await launchDesktop('real', 'metahub-remote-question')
    await ready(desktop.page)
    await joinDesktop(desktop, meta)
    remote = await startRemoteHub(desktop, meta)
    await expect.poll(() => cluster(desktop!.page), { timeout: 15_000 }).toEqual({
      state: 'connected', hubs: ['hub-a', 'hub-b'], remoteMain: 'running',
    })
    remote.probe.close()
    await desktop.page.waitForTimeout(500)

    await send(desktop.page, 'Spawn the remote question child')
    const questions = desktop.page.getByTestId('questions-pane')
    await expect(questions).toContainText('Choose remote verification', { timeout: 20_000 })
    await expect(questions).toContainText('Agent question · hub-b/remote-question')
    await questions.getByRole('button', { name: /Fast/ }).click()
    await questions.getByRole('button', { name: 'Submit answers' }).click()
    await expect(questions).toHaveCount(0)
    await expect(desktop.page.getByTestId('conversation')).toContainText(
      'Root observed the remote question answer.', { timeout: 20_000 },
    )
    await expect.poll(() => child(desktop!.page), { timeout: 15_000 }).toMatchObject({
      id: 'hub-b/remote-question', status: 'completed', controllable: false,
    })

    await expect.poll(() => desktop!.llm!.state()).toMatchObject({
      served: 4, remaining: 0, requestCount: 4, unmatchedRequests: 0,
      inFlight: 0, verified: true,
    })
    const requests = await desktop.llm!.requests()
    expect(requests).toHaveLength(4)
    expect(requests.every((request) => request.matched)).toBe(true)
    expect(requests[2]!.toolResultIds).toContain('remote-ask')
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
  return (await detailOf(page)).agents.find((agent) => agent.id === 'hub-b/remote-question')
}

async function detailOf(page: Page) {
  return page.evaluate(async () => {
    const bootstrap = await window.loopalDesktop.bootstrap()
    return window.loopalDesktop.openSession(bootstrap.activeSessionId!)
  })
}
