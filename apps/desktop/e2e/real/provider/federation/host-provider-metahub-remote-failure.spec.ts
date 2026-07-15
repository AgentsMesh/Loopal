import { expect, test, type Page } from '@playwright/test'
import { closeDesktop, launchDesktop, type DesktopFixture } from '../../../support/electron/electron-fixture'
import { ready, send } from '../../../support/runtime/llm-e2e-helpers'
import {
  startMetaHub, startRemoteHub, stopProcess,
  type MetaHubProcess, type RemoteHubProcess,
} from '../../../support/federation/metahub-data-plane-fixture'

test('projects a remote child provider failure and returns it to root', async () => {
  let meta: MetaHubProcess | undefined
  let desktop: DesktopFixture | undefined
  let remote: RemoteHubProcess | undefined
  try {
    meta = await startMetaHub()
    desktop = await launchDesktop('real', 'metahub-remote-failure')
    await ready(desktop.page)
    await joinDesktop(desktop, meta)
    remote = await startRemoteHub(desktop, meta)
    await expect.poll(() => remoteMain(desktop!.page), { timeout: 15_000 }).toBe('running')

    await send(desktop.page, 'Spawn the remote provider failure child')
    const conversation = desktop.page.getByTestId('conversation')
    await expect(conversation).toContainText(
      'Root observed the remote provider failure.', { timeout: 20_000 },
    )
    await expect.poll(() => failedChild(desktop!.page), { timeout: 15_000 }).toEqual({
      projected: {
        id: 'hub-b/remote-failed', status: 'failed', controllable: false,
        error: expect.stringContaining('scripted remote child failure'),
      },
      topology: {
        id: 'hub-b/remote-failed', lifecycle: 'failed',
        error: expect.stringContaining('scripted remote child failure'),
      },
    })
    const agent = conversation.getByTestId('tool-invocation').filter({ hasText: /^Agent/ })
    await expect(agent).toHaveCount(1)
    await expect(agent).toContainText('scripted remote child failure')

    await expect.poll(() => desktop!.llm!.state()).toMatchObject({
      served: 3, remaining: 0, requestCount: 3,
      unmatchedRequests: 0, inFlight: 0, verified: true,
    })
    expect((await desktop.llm!.requests()).every((request) => request.matched)).toBe(true)
  } finally {
    remote?.probe.close()
    if (remote) await stopProcess(remote.child)
    if (desktop) await closeDesktop(desktop)
    if (meta) await stopProcess(meta.child)
  }
})

async function joinDesktop(desktop: DesktopFixture, meta: MetaHubProcess): Promise<void> {
  await desktop.page.evaluate(async ({ address, token }) => {
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
  }, { address: meta.address, token: meta.token })
}

async function detail(page: Page) {
  return page.evaluate(async () => {
    const bootstrap = await window.loopalDesktop.bootstrap()
    return window.loopalDesktop.openSession(bootstrap.activeSessionId!)
  })
}

async function remoteMain(page: Page): Promise<string | undefined> {
  return (await detail(page)).metaHub?.topology
    .find((agent) => agent.id === 'hub-b/main')?.lifecycle
}

async function failedChild(page: Page) {
  const value = await detail(page)
  const projected = value.agents.find((agent) => agent.id === 'hub-b/remote-failed')
  const topology = value.metaHub?.topology.find((agent) => agent.id === 'hub-b/remote-failed')
  return {
    projected: projected && {
      id: projected.id, status: projected.status,
      controllable: projected.controllable, error: projected.error,
    },
    topology: topology && {
      id: topology.id, lifecycle: topology.lifecycle, error: topology.error,
    },
  }
}
