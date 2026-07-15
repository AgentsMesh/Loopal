import { expect, test } from '@playwright/test'
import { closeDesktop, launchDesktop, type DesktopFixture } from '../../../support/electron/electron-fixture'
import { ready } from '../../../support/runtime/llm-e2e-helpers'
import {
  startMetaHub, startRemoteHub, stopProcess,
  type MetaHubProcess, type RemoteHubProcess,
} from '../../../support/federation/metahub-data-plane-fixture'

test('routes model turns across MetaHub once before and after reconnect', async () => {
  let meta: MetaHubProcess | undefined
  let desktop: DesktopFixture | undefined
  let remote: RemoteHubProcess | undefined
  try {
    meta = await startMetaHub()
    desktop = await launchDesktop('real', 'metahub-data-plane')
    await ready(desktop.page)
    const target = await joinDesktop(desktop, meta)
    remote = await startRemoteHub(desktop, meta)
    await expect.poll(async () => topologyConnection(desktop!, target.sessionId), {
      timeout: 15_000,
    }).toEqual({
      state: 'connected', localHub: 'hub-a', hubs: ['hub-a', 'hub-b'],
      remote: {
        id: 'hub-b/main', name: 'main', hub: 'hub-b', hubPath: ['hub-b'],
        lifecycle: 'running',
      },
    })

    await exchange(desktop, remote, 'ONE', 5)
    const reconnect = await desktop.page.evaluate(async (value) => {
      const disconnected = await window.loopalDesktop.disconnectMetaHub(value)
      const joined = await window.loopalDesktop.joinMetaHub(value)
      return { disconnected: disconnected.state, joined: joined.state }
    }, target)
    expect(reconnect).toEqual({ disconnected: 'disconnected', joined: 'connected' })
    await exchange(desktop, remote, 'TWO', 10)

    const requests = await desktop.llm!.requests()
    for (const marker of [
      '[from: hub-b/main] REMOTE REQUEST ONE',
      '[from: hub-a/main] DESKTOP MODEL REPLY ONE',
      '[from: hub-b/main] REMOTE REQUEST TWO',
      '[from: hub-a/main] DESKTOP MODEL REPLY TWO',
    ]) {
      expect(requests.filter((request) => request.lastUserText.includes(marker))).toHaveLength(1)
    }
    await expect.poll(() => desktop!.llm!.state()).toMatchObject({
      served: 10, remaining: 0, unmatchedRequests: 0, verified: true,
    })
  } finally {
    remote?.probe.close()
    if (remote) await stopProcess(remote.child)
    if (desktop) await closeDesktop(desktop)
    if (meta) await stopProcess(meta.child)
  }
})

test('delivers one reply when MetaHub reconnects during a model turn', async () => {
  let meta: MetaHubProcess | undefined
  let desktop: DesktopFixture | undefined
  let remote: RemoteHubProcess | undefined
  try {
    meta = await startMetaHub()
    desktop = await launchDesktop('real', 'metahub-inflight-reconnect')
    await ready(desktop.page)
    const target = await joinDesktop(desktop, meta)
    remote = await startRemoteHub(desktop, meta)
    await expect.poll(async () => topologyConnection(desktop!, target.sessionId), {
      timeout: 15_000,
    }).toMatchObject({ state: 'connected', hubs: ['hub-a', 'hub-b'] })

    await remote.probe.startModelTurn('REMOTE INFLIGHT INITIATE')
    await expect(desktop.page.locator('[data-message-role="user"]')
      .filter({ hasText: 'REMOTE INFLIGHT REQUEST' })).toHaveCount(1)
    await expect.poll(() => desktop!.llm!.state(), { timeout: 10_000 })
      .toMatchObject({ inFlight: 1 })
    const reconnect = await desktop.page.evaluate(async (value) => {
      const disconnected = await window.loopalDesktop.disconnectMetaHub(value)
      const joined = await window.loopalDesktop.joinMetaHub(value)
      return [disconnected.state, joined.state]
    }, target)
    expect(reconnect).toEqual(['disconnected', 'connected'])

    await expect(desktop.page.getByTestId('conversation')).toContainText(
      'DESKTOP INFLIGHT ACK', { timeout: 20_000 },
    )
    await expect.poll(() => remoteInboxCount(remote!, 'DESKTOP INFLIGHT REPLY'), {
      timeout: 15_000,
    }).toBe(1)
    await expect.poll(() => desktop!.llm!.state()).toMatchObject({
      served: 5, remaining: 0, inFlight: 0, unmatchedRequests: 0, verified: true,
    })
    await desktop.page.waitForTimeout(1_000)
    expect(remoteInboxCount(remote, 'DESKTOP INFLIGHT REPLY')).toBe(1)
    const requests = await desktop.llm!.requests()
    expect(requests.filter((request) => request.lastUserText.includes(
      '[from: hub-b/main] REMOTE INFLIGHT REQUEST',
    ))).toHaveLength(1)
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
    const runtime = bootstrap.runtimes.find((value) => value.sessionId === sessionId)!
    const target = {
      sessionId, runtimeId: runtime.id, generation: runtime.generation,
      agentId: runtime.rootAgent,
    }
    await api.controlAgent({ target, command: { type: 'permission', mode: 'bypass' } })
    await api.updateMetaHubSettings({
      address, hubName: 'hub-a', token, joinOnStart: false, startLocalOnLaunch: false,
    })
    await api.joinMetaHub(target)
    return target
  }, { address: meta.address, token: meta.token })
}

async function exchange(
  desktop: DesktopFixture, remote: RemoteHubProcess, suffix: 'ONE' | 'TWO', served: number,
): Promise<void> {
  await remote.probe.startModelTurn(`REMOTE INITIATE ${suffix}`)
  const messages = desktop.page.locator('[data-message-role="user"]')
  const incoming = messages.filter({ hasText: `REMOTE REQUEST ${suffix}` })
  await expect(incoming).toHaveCount(1)
  await expect(incoming).toContainText('From · hub-b/main')
  await expect(desktop.page.getByTestId('conversation')).toContainText(
    `DESKTOP LOCAL ACK ${suffix}`, { timeout: 15_000 },
  )
  await expect.poll(() => remoteInboxCount(remote, `DESKTOP MODEL REPLY ${suffix}`), {
    timeout: 15_000,
  }).toBe(1)
  await expect.poll(() => desktop.llm!.state()).toMatchObject({ served })
}

function remoteInboxCount(remote: RemoteHubProcess, marker: string): number {
  return remote.probe.notifications().filter((value) => {
    const encoded = JSON.stringify(value)
    return encoded.includes('InboxEnqueued')
      && encoded.includes('hub-a') && encoded.includes(marker)
  }).length
}

async function topologyConnection(desktop: DesktopFixture, sessionId: string) {
  const detail = await desktop.page.evaluate(
    (value) => window.loopalDesktop.openSession(value), sessionId,
  )
  const state = detail.metaHub
  const remote = state?.topology.find((agent) => agent.id === 'hub-b/main')
  return {
    state: state?.state,
    localHub: state?.hubName,
    hubs: state?.hubs.map((hub) => hub.name).sort(),
    remote: remote ? {
      id: remote.id, name: remote.name, hub: remote.hub,
      hubPath: remote.hubPath, lifecycle: remote.lifecycle,
    } : undefined,
  }
}
