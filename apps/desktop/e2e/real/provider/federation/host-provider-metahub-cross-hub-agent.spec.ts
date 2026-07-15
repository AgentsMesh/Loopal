import { expect, test, type Locator, type Page } from '@playwright/test'
import {
  closeDesktop, launchDesktop, relaunchDesktop, type DesktopFixture,
  waitForHostStatus,
} from '../../../support/electron/electron-fixture'
import { ready, runtimeTarget, send } from '../../../support/runtime/llm-e2e-helpers'
import {
  startMetaHub, startRemoteHub, stopProcess,
  type MetaHubProcess, type RemoteHubProcess,
} from '../../../support/federation/metahub-data-plane-fixture'

const childResult = 'REMOTE CHILD PROVIDER COMPLETION 7E92'

test('auto-joins and completes a model-driven Agent on a remote Hub once', async () => {
  let meta: MetaHubProcess | undefined
  let desktop: DesktopFixture | undefined
  let remote: RemoteHubProcess | undefined
  try {
    meta = await startMetaHub()
    desktop = await launchDesktop('real', 'metahub-cross-hub-agent')
    await ready(desktop.page)
    remote = await startRemoteHub(desktop, meta)
    await saveAutoJoin(desktop.page, meta)

    desktop = await relaunchDesktop(desktop)
    await readyAfterAutoJoin(desktop)
    await expect.poll(() => cluster(desktop!.page), { timeout: 15_000 }).toEqual({
      state: 'connected', hubName: expect.stringMatching(/^hub-a-.+-g\d+-[a-z0-9]+$/),
      hubs: [expect.stringMatching(/^hub-a-.+-g\d+-[a-z0-9]+$/), 'hub-b'],
      remoteMain: 'running',
    })

    const before = await runtimeTarget(desktop.page)
    const restarted = await desktop.page.evaluate(
      (sessionId) => window.loopalDesktop.restartSession(sessionId), before.sessionId,
    )
    expect(restarted.generation).toBeGreaterThan(before.generation)
    await ready(desktop.page)
    await expect.poll(() => cluster(desktop!.page), { timeout: 15_000 }).toEqual({
      state: 'connected', hubName: expect.stringMatching(/^hub-a-.+-g\d+-[a-z0-9]+$/),
      hubs: [expect.stringMatching(/^hub-a-.+-g\d+-[a-z0-9]+$/), 'hub-b'],
      remoteMain: 'running',
    })

    const target = await runtimeTarget(desktop.page)
    expect(target.generation).toBe(restarted.generation)
    await desktop.page.evaluate(async (value) => window.loopalDesktop.controlAgent({
      target: value, command: { type: 'permission', mode: 'bypass' },
    }), target)
    await send(desktop.page, 'Exercise the cross-hub Agent lifecycle')

    const conversation = desktop.page.getByTestId('conversation')
    await expect(conversation).toContainText('Remote child status is running.', {
      timeout: 10_000,
    })
    await desktop.page.getByRole('button', { name: 'Federation', exact: true }).click()
    const remoteNode = desktop.page.getByTestId('federation-workspace')
      .locator('[data-qualified-agent-id="hub-b/remote-worker"]')
    await expect(remoteNode).toBeVisible({ timeout: 8_000 })
    await expect(remoteNode).toContainText('running')
    await expect.poll(() => projectedChild(desktop!.page), { timeout: 8_000 }).toEqual([
      qualifiedChild('running'),
    ])

    await desktop.page.getByRole('button', { name: 'Conversation', exact: true }).click()
    await expect(conversation).toContainText(
      'Cross-hub result observed exactly once.', { timeout: 20_000 },
    )
    await desktop.page.getByRole('button', { name: 'Federation', exact: true }).click()
    await expect(remoteNode).toContainText('remote-worker', { timeout: 10_000 })
    await expect(remoteNode).toHaveAttribute('data-lifecycle', 'finished', { timeout: 10_000 })
    await expect(desktop.page.getByTestId('federation-workspace')
      .locator('[data-qualified-agent-id="hub-a/remote-worker"]')).toHaveCount(0)
    await expect.poll(() => projectedChild(desktop!.page), { timeout: 10_000 }).toEqual([
      qualifiedChild('completed'),
    ])
    await desktop.page.getByRole('button', { name: 'Conversation', exact: true }).click()
    const completion = conversation.locator('[data-message-role="user"]')
      .filter({ hasText: childResult })
    await expect(completion).toHaveCount(1)
    await expect(completion).toContainText('From · hub-b/remote-worker')
    await assertToolChain(conversation)

    await expect.poll(() => desktop!.llm!.state()).toMatchObject({
      served: 7, remaining: 0, requestCount: 7, unmatchedRequests: 0,
      inFlight: 0, verified: true,
    })
    await desktop.page.waitForTimeout(1_500)
    expect(await desktop.llm!.state()).toMatchObject({
      served: 7, remaining: 0, requestCount: 7, unmatchedRequests: 0,
      inFlight: 0, verified: true,
    })
    const requests = await desktop.llm!.requests()
    expect(requests).toHaveLength(7)
    expect(requests.every((request) => request.matched)).toBe(true)
    expect(requests.filter((request) => (
      request.lastUserText.includes(`Return ${childResult}`)
    ))).toHaveLength(1)
  } finally {
    remote?.probe.close()
    if (remote) await stopProcess(remote.child)
    if (desktop) await closeDesktop(desktop)
    if (meta) await stopProcess(meta.child)
  }
})

async function saveAutoJoin(page: Page, meta: MetaHubProcess): Promise<void> {
  const saved = await page.evaluate(async ({ address, token }) => (
    window.loopalDesktop.updateMetaHubSettings({
      address, hubName: 'hub-a', token, joinOnStart: true, startLocalOnLaunch: false,
    })
  ), { address: meta.address, token: meta.token })
  expect(saved).toMatchObject({
    hubName: 'hub-a', joinOnStart: true, tokenConfigured: true,
  })
}

async function readyAfterAutoJoin(desktop: DesktopFixture): Promise<void> {
  const page = desktop.page
  await waitForHostStatus(page, 'ready')
  try {
    await expect(page.getByTestId('runtime-status')).toContainText(
      'Ready for input', { timeout: 15_000 },
    )
    await expect(page.getByLabel('Message Loopal')).toBeEnabled({ timeout: 5_000 })
  } catch (error) {
    const state = await desktop.llm!.state()
    const requests = (await desktop.llm!.requests()).slice(-8).map((request) => ({
      sequence: request.sequence, matched: request.matched,
      lastUserText: request.lastUserText.slice(0, 300),
      toolResultIds: request.toolResultIds,
    }))
    const runtime = await page.evaluate(async () => {
      const bootstrap = await window.loopalDesktop.bootstrap()
      const detail = await window.loopalDesktop.openSession(bootstrap.activeSessionId!)
      return {
        session: detail.session, metaHub: detail.metaHub,
        agents: detail.agents.map((agent) => ({
          id: agent.id, status: agent.status, error: agent.error,
          telemetry: agent.telemetry,
        })),
        conversation: detail.conversation.slice(-8).map((entry) => ({
          role: entry.role, text: entry.text.slice(0, 300), inbox: entry.inbox,
        })),
      }
    })
    throw new Error(`auto-join runtime did not become idle: ${JSON.stringify({ state, requests, runtime })}`, { cause: error })
  }
}

async function cluster(page: Page) {
  return page.evaluate(async () => {
    const bootstrap = await window.loopalDesktop.bootstrap()
    const detail = await window.loopalDesktop.openSession(bootstrap.activeSessionId!)
    return {
      state: detail.metaHub?.state,
      hubName: detail.metaHub?.hubName,
      hubs: detail.metaHub?.hubs.map((hub) => hub.name).sort() ?? [],
      remoteMain: detail.metaHub?.topology.find((agent) => agent.id === 'hub-b/main')?.lifecycle,
    }
  })
}

async function projectedChild(page: Page) {
  return page.evaluate(async () => {
    const bootstrap = await window.loopalDesktop.bootstrap()
    const detail = await window.loopalDesktop.openSession(bootstrap.activeSessionId!)
    return detail.agents.filter((agent) => (
      agent.id === 'remote-worker' || agent.id === 'hub-b/remote-worker'
    )).map((agent) => ({
      id: agent.id, qualifiedName: agent.qualifiedName, status: agent.status,
      parentId: agent.parentId, hubPath: agent.hubPath, controllable: agent.controllable,
    }))
  })
}

function qualifiedChild(status: 'running' | 'completed') {
  return {
    id: 'hub-b/remote-worker', qualifiedName: 'hub-b/remote-worker', status,
    parentId: 'main', hubPath: ['hub-b'], controllable: status === 'running',
  }
}

async function assertToolChain(conversation: Locator): Promise<void> {
  const tools = conversation.getByTestId('tool-invocation')
  const listed = tools.filter({ hasText: /^ListHubs/ })
  await expect(listed).toHaveCount(1)
  await expect(listed).toContainText("Connected to MetaHub as 'hub-a-")
  const agent = tools.filter({ hasText: /^Agent/ })
  await expect(agent).toHaveCount(3)
  await expect(agent.filter({ hasText: '"target_hub": "hub-b"' })).toHaveCount(1)
  await expect(agent.filter({ hasText: '"action": "status"' })).toContainText('Running')
  await expect(agent.filter({ hasText: '"action": "result"' })).toContainText(childResult)
  for (let index = 0; index < 3; index += 1) await expect(agent.nth(index)).toContainText('Completed')
}
