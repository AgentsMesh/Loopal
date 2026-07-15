import { expect, test, type Page } from '@playwright/test'
import {
  closeDesktop, launchDesktop, type DesktopFixture,
} from '../support/electron/electron-fixture'

test('runs Federation independently and manages two sessions from their context menus', async () => {
  const desktop = await launchDesktop('fake')
  try {
    const page = desktop.page
    await page.setViewportSize({ width: 1440, height: 900 })
    await expectBackground(desktop)
    const selected = page.locator('.session-card.selected')
    const activeId = await requiredAttribute(selected, 'data-session-id')
    const other = page.locator('.session-card:not(.selected)').first()
    const otherId = await requiredAttribute(other, 'data-session-id')

    await page.getByRole('button', { name: 'Federation', exact: true }).click()
    const workspace = page.getByTestId('primary-workspace')
    await expect(workspace).toHaveAttribute('data-workspace', 'federation')
    await expect(workspace.locator('.session-toolbar')).toHaveCount(0)
    await expect(workspace.locator('.composer-lifecycle')).toHaveCount(0)
    await expect(workspace.getByTestId('host-status')).toHaveCount(0)
    await expect(workspace.getByTestId('runtime-status')).toHaveCount(0)
    await expect(workspace.getByTestId('active-session-title')).toHaveCount(0)
    await expect(workspace.getByRole('button', { name: 'Stop', exact: true })).toHaveCount(0)
    await expect(workspace.getByRole('button', { name: 'Restart', exact: true })).toHaveCount(0)
    await expect(workspace.getByText('Workspace', { exact: true })).toHaveCount(0)
    await expect(workspace.getByText('Session', { exact: true })).toHaveCount(0)

    await page.getByTestId('federation-start').click()
    await expect(page.getByTestId('federation-local-state')).toHaveText('running')
    await page.getByRole('button', { name: 'Conversation', exact: true }).click()

    await selected.focus()
    await page.keyboard.press('Shift+F10')
    await expect(page.getByTestId('session-context-menu')).toBeVisible()
    await page.keyboard.press('Escape')
    await expect(page.getByTestId('session-context-menu')).toHaveCount(0)
    await expect(page.locator('.session-card.selected')).toHaveAttribute('data-session-id', activeId)

    await joinFromMenu(page, activeId)
    await expectConnected(page, activeId, true)
    await expect(page.locator('.session-card.selected')).toHaveAttribute('data-session-id', activeId)
    await joinFromMenu(page, otherId)
    await expectConnected(page, otherId, true)
    await expect(page.locator('.session-card.selected')).toHaveAttribute('data-session-id', activeId)

    await page.getByRole('button', { name: 'Federation', exact: true }).click()
    await expect(page.getByTestId('federation-connection')).toContainText('2 sessions joined')
    await page.getByRole('button', { name: 'Conversation', exact: true }).click()
    await leaveFromMenu(page, activeId)
    await expectConnected(page, activeId, false)
    await expectConnected(page, otherId, true)
    await expect(page.locator('.session-card.selected')).toHaveAttribute('data-session-id', activeId)

    await page.getByRole('button', { name: 'Federation', exact: true }).click()
    await expect(page.getByTestId('federation-connection')).toContainText('1 sessions joined')
    const owner = page.locator(`[data-owner-session-id="${otherId}"]`).first()
    await expect(owner).toBeVisible()
    await owner.click()
    await page.getByRole('button', { name: 'Open conversation' }).click()
    await expect(page.getByTestId('primary-workspace')).toHaveAttribute(
      'data-workspace', 'conversation',
    )
    await expect(page.locator('.session-card.selected')).toHaveAttribute('data-session-id', otherId)
    await expectBackground(desktop)
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

async function expectConnected(page: Page, sessionId: string, connected: boolean): Promise<void> {
  await expect.poll(() => page.evaluate(async (id) => {
    const api = window.loopalDesktop
    const bootstrap = await api.bootstrap()
    const session = bootstrap.sessions.find((item) => item.id === id)!
    const runtime = bootstrap.runtimes.find((item) => item.id === session.activeRuntimeId)!
    return (await api.getMetaHubStatus({
      sessionId: id, runtimeId: runtime.id, generation: runtime.generation,
    })).state
  }, sessionId)).toBe(connected ? 'connected' : 'disconnected')
}

async function requiredAttribute(
  locator: import('@playwright/test').Locator, name: string,
): Promise<string> {
  const value = await locator.getAttribute(name)
  if (!value) throw new Error(`Missing ${name}`)
  return value
}

async function expectBackground(desktop: DesktopFixture): Promise<void> {
  expect(await desktop.app.evaluate(({ BrowserWindow }) => {
    const window = BrowserWindow.getAllWindows()[0]
    return { visible: window?.isVisible(), focused: window?.isFocused() }
  })).toEqual({ visible: false, focused: false })
}
