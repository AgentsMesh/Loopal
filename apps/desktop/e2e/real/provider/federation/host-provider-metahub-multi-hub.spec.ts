import { expect, test, type Locator, type Page } from '@playwright/test'
import { closeDesktop, launchDesktop, type DesktopFixture } from '../../../support/electron/electron-fixture'
import { ready, runtimeTarget, send } from '../../../support/runtime/llm-e2e-helpers'
import {
  startMetaHub, startRemoteHub, stopProcess,
  type MetaHubProcess, type RemoteHubProcess,
} from '../../../support/federation/metahub-data-plane-fixture'

const results = {
  'worker-b': 'REMOTE HUB B RESULT 84B7',
  'worker-c': 'REMOTE HUB C RESULT 52C9',
} as const

test('runs two remote Agents on independent Hubs and completes each once', async () => {
  let meta: MetaHubProcess | undefined
  let desktop: DesktopFixture | undefined
  let hubB: RemoteHubProcess | undefined
  let hubC: RemoteHubProcess | undefined
  try {
    meta = await startMetaHub()
    desktop = await launchDesktop('real', 'metahub-multi-hub-agent')
    await ready(desktop.page)
    ;[hubB, hubC] = await Promise.all([
      startRemoteHub(desktop, meta, 'hub-b'),
      startRemoteHub(desktop, meta, 'hub-c'),
    ])
    assertIndependentProcesses(hubB, hubC)
    const target = await connect(desktop.page, meta)
    await expect.poll(() => cluster(desktop!.page), { timeout: 15_000 }).toEqual({
      state: 'connected', hubName: 'hub-a', hubs: ['hub-a', 'hub-b', 'hub-c'],
      mains: ['hub-b/main:running', 'hub-c/main:running'],
    })

    await send(desktop.page, 'Exercise two remote Hubs concurrently')
    await expect.poll(() => children(desktop!.page), { timeout: 8_000 }).toEqual([
      expectedChild('hub-b', 'worker-b', 'running', true),
      expectedChild('hub-c', 'worker-c', 'running', true),
    ])
    await assertRawTopology(desktop.page, 'running')
    await expect(desktop.page.getByTestId('conversation')).toContainText(
      'Both remote agents are running.', { timeout: 20_000 },
    )

    const conversation = desktop.page.getByTestId('conversation')
    await expect(conversation).toContainText('Hub B result observed exactly once.', {
      timeout: 20_000,
    })
    await expect(conversation).toContainText('Hub C result observed exactly once.', {
      timeout: 20_000,
    })
    await expect.poll(() => children(desktop!.page), { timeout: 10_000 }).toEqual([
      expectedChild('hub-b', 'worker-b', 'completed', false),
      expectedChild('hub-c', 'worker-c', 'completed', false),
    ])
    await assertRawTopology(desktop.page, 'finished')
    await assertExactlyOnce(conversation)
    await assertTools(conversation)

    await expect.poll(() => desktop!.llm!.state()).toMatchObject({
      served: 8, remaining: 0, requestCount: 8, unmatchedRequests: 0,
      inFlight: 0, verified: true,
    })
    const requests = await desktop.llm!.requests()
    expect(requests).toHaveLength(8)
    expect(requests.every((request) => request.matched)).toBe(true)
    for (const result of Object.values(results)) {
      expect(requests.filter((request) => request.lastUserText.includes(`Return ${result}`)))
        .toHaveLength(1)
    }
    expect(target.agentId).toBe('main')
  } finally {
    hubB?.probe.close(); hubC?.probe.close()
    if (hubB) await stopProcess(hubB.child)
    if (hubC) await stopProcess(hubC.child)
    if (desktop) await closeDesktop(desktop)
    if (meta) await stopProcess(meta.child)
  }
})

function assertIndependentProcesses(a: RemoteHubProcess, b: RemoteHubProcess): void {
  expect(a.child.pid).toBeGreaterThan(0)
  expect(b.child.pid).toBeGreaterThan(0)
  expect(a.child.pid).not.toBe(b.child.pid)
  expect(() => process.kill(a.child.pid!, 0)).not.toThrow()
  expect(() => process.kill(b.child.pid!, 0)).not.toThrow()
}

async function connect(page: Page, meta: MetaHubProcess) {
  const target = await runtimeTarget(page)
  await page.evaluate(async ({ target: value, address, token }) => {
    await window.loopalDesktop.controlAgent({
      target: value, command: { type: 'permission', mode: 'bypass' },
    })
    await window.loopalDesktop.updateMetaHubSettings({
      address, hubName: 'hub-a', token, joinOnStart: false, startLocalOnLaunch: false,
    })
    await window.loopalDesktop.joinMetaHub(value)
  }, { target, address: meta.address, token: meta.token })
  return target
}

async function cluster(page: Page) {
  const detail = await active(page)
  return {
    state: detail.metaHub?.state, hubName: detail.metaHub?.hubName,
    hubs: detail.metaHub?.hubs.map((hub) => hub.name).sort(),
    mains: detail.metaHub?.topology.filter((agent) => (
      agent.id === 'hub-b/main' || agent.id === 'hub-c/main'
    )).map((agent) => `${agent.id}:${agent.lifecycle}`).sort(),
  }
}

async function children(page: Page) {
  const detail = await active(page)
  return detail.agents.filter((agent) => agent.id.includes('/worker-')).map((agent) => ({
    id: agent.id, qualifiedName: agent.qualifiedName, parentId: agent.parentId,
    hubPath: agent.hubPath, status: agent.status, controllable: agent.controllable,
  })).sort((a, b) => a.id.localeCompare(b.id))
}

function expectedChild(
  hub: 'hub-b' | 'hub-c', name: 'worker-b' | 'worker-c',
  status: 'running' | 'completed', controllable: boolean,
) {
  return {
    id: `${hub}/${name}`, qualifiedName: `${hub}/${name}`, parentId: 'main',
    hubPath: [hub], status, controllable,
  }
}

async function assertRawTopology(page: Page, lifecycle: 'running' | 'finished'): Promise<void> {
  const topology = (await active(page)).metaHub!.topology
  for (const [hub, name] of [['hub-b', 'worker-b'], ['hub-c', 'worker-c']] as const) {
    expect(topology.find((agent) => agent.id === `${hub}/${name}`)).toMatchObject({
      name, hub, hubPath: [hub], parentId: 'hub-a/main', lifecycle,
    })
  }
}

async function assertExactlyOnce(conversation: Locator): Promise<void> {
  for (const [name, result] of Object.entries(results)) {
    const completion = conversation.locator('[data-message-role="user"]').filter({ hasText: result })
    await expect(completion).toHaveCount(1)
    await expect(completion).toContainText(`From · ${name === 'worker-b' ? 'hub-b' : 'hub-c'}/${name}`)
  }
}

async function assertTools(conversation: Locator): Promise<void> {
  const agents = conversation.getByTestId('tool-invocation').filter({ hasText: /^Agent/ })
  await expect(agents).toHaveCount(4)
  for (const [hub, name] of [['hub-b', 'worker-b'], ['hub-c', 'worker-c']] as const) {
    await expect(agents.filter({ hasText: `"target_hub": "${hub}"` })).toHaveCount(1)
    await expect(agents.filter({ hasText: `"name": "${name}"` })).toHaveCount(2)
  }
}

async function active(page: Page) {
  return page.evaluate(async () => {
    const bootstrap = await window.loopalDesktop.bootstrap()
    return window.loopalDesktop.openSession(bootstrap.activeSessionId!)
  })
}
