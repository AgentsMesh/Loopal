import { expect, test, type Page } from '@playwright/test'
import {
  closeDesktop, launchDesktop, relaunchDesktop, type DesktopFixture, waitForHostStatus,
} from '../../../support/electron/electron-fixture'
import {
  createSessionDirectory, queueSessionDirectories,
} from '../../../support/fixtures/session-directory-fixture'
import { createFromDirectory } from '../../../support/fixtures/session-directory-ui'

interface FederationRuntime {
  readonly sessionId: string
  readonly runtimeId: string
  readonly generation: number
}

test('persists a real Federation while two exact runtimes join and leave independently', async () => {
  let desktop = await launchDesktop('real')
  try {
    const page = desktop.page
    await waitForHostStatus(page, 'ready')
    await expectBackground(desktop)
    await page.evaluate(async () => {
      const current = await window.loopalDesktop.getMetaHubSettings()
      await window.loopalDesktop.updateMetaHubSettings({
        address: '', hubName: current.hubName,
        joinOnStart: true, startLocalOnLaunch: false,
      })
    })
    await page.getByRole('button', { name: 'Federation', exact: true }).click()
    await page.getByTestId('federation-start').click()
    await expect(page.getByTestId('federation-local-state')).toHaveText('running', {
      timeout: 15_000,
    })
    await page.getByRole('button', { name: 'Conversation', exact: true }).click()

    const firstId = await selectedSessionId(page)
    const secondDirectory = await createSessionDirectory(desktop, 'federated-host', false)
    await queueSessionDirectories(desktop, [secondDirectory.path])
    await createFromDirectory(page, 'directory')
    await expect(page.locator('.session-card')).toHaveCount(2, { timeout: 30_000 })
    const secondId = await selectedSessionId(page)
    expect(secondId).not.toBe(firstId)
    await joinFromMenu(page, firstId)
    await joinFromMenu(page, secondId)

    await expect.poll(() => federationState(page, [firstId, secondId]), {
      timeout: 20_000,
    }).toMatchObject({
      local: { state: 'running', address: expect.any(String) },
      settings: { startLocalOnLaunch: true, joinOnStart: false },
      states: [
        { state: 'connected', hubName: expect.any(String), topologySize: 2 },
        { state: 'connected', hubName: expect.any(String), topologySize: 2 },
      ],
    })
    const joined = await federationState(page, [firstId, secondId])
    expect(joined.states[0]!.hubName).not.toBe(joined.states[1]!.hubName)

    await leaveFromMenu(page, firstId)
    await expect.poll(() => federationState(page, [firstId, secondId]), {
      timeout: 15_000,
    }).toMatchObject({
      local: { state: 'running' },
      states: [{ state: 'disconnected' }, { state: 'connected' }],
    })

    desktop = await relaunchDesktop(desktop)
    await expectBackground(desktop)
    await waitForHostStatus(desktop.page, 'ready')
    await expect.poll(() => federationState(desktop.page, [firstId, secondId]), {
      timeout: 30_000,
    }).toMatchObject({
      local: { state: 'running', address: expect.any(String) },
      settings: { startLocalOnLaunch: true, joinOnStart: false },
      connectedCount: 0, liveCount: 1,
    })
  } finally {
    await closeDesktop(desktop)
  }
})

async function joinFromMenu(page: Page, sessionId: string): Promise<void> {
  await page.locator(`[data-session-id="${sessionId}"]`).click({ button: 'right' })
  const menu = page.getByTestId('session-context-menu')
  await expect(menu).toBeVisible()
  await menu.getByRole('menuitem', { name: 'Join Federation' }).click()
  await expect(menu).toHaveCount(0)
}

async function leaveFromMenu(page: Page, sessionId: string): Promise<void> {
  await page.locator(`[data-session-id="${sessionId}"]`).click({ button: 'right' })
  const menu = page.getByTestId('session-context-menu')
  await expect(menu).toBeVisible()
  await menu.getByRole('menuitem', { name: 'Leave Federation' }).click()
  await expect(menu).toHaveCount(0)
}

async function selectedSessionId(page: Page): Promise<string> {
  const value = await page.locator('.session-card.selected').getAttribute('data-session-id')
  if (!value) throw new Error('No selected session')
  return value
}

async function federationState(page: Page, sessionIds: readonly string[]) {
  return page.evaluate(async (ids) => {
    const api = window.loopalDesktop
    const bootstrap = await api.bootstrap()
    const targets = ids.map((sessionId): FederationRuntime | undefined => {
      const session = bootstrap.sessions.find((item) => item.id === sessionId)
      const runtime = bootstrap.runtimes.find((item) => item.id === session?.activeRuntimeId)
      if (!runtime) return undefined
      return { sessionId, runtimeId: runtime.id, generation: runtime.generation }
    })
    const states = await Promise.all(targets.map(async (target) => {
      if (!target) return { state: 'unavailable', topologySize: 0 }
      const state = await api.getMetaHubStatus(target)
      return {
        state: state.state, hubName: state.hubName,
        topologySize: state.topology.length,
      }
    }))
    return {
      local: await api.getLocalMetaHubStatus(),
      settings: await api.getMetaHubSettings(), states,
      connectedCount: states.filter(({ state }) => state === 'connected').length,
      liveCount: states.filter(({ state }) => state !== 'unavailable').length,
    }
  }, sessionIds)
}

async function expectBackground(desktop: DesktopFixture): Promise<void> {
  expect(await desktop.app.evaluate(({ BrowserWindow }) => {
    const window = BrowserWindow.getAllWindows()[0]
    return { visible: window?.isVisible(), focused: window?.isFocused() }
  })).toEqual({ visible: false, focused: false })
}
