import { expect, test } from '@playwright/test'
import {
  closeDesktop, launchDesktop, type DesktopFixture,
} from '../support/electron/electron-fixture'
import { selectSettingsSection } from '../support/settings/settings-helpers'

test('joins MetaHub and exposes the connected Federation workspace', async () => {
  const desktop = await launchDesktop('fake')
  try {
    const page = desktop.page
    await page.setViewportSize({ width: 1440, height: 900 })
    await expectBackground(desktop)
    const workspace = page.getByTestId('primary-workspace')
    await expect(workspace).toHaveAttribute('data-workspace', 'conversation')

    await page.getByRole('button', { name: 'Settings' }).click()
    await selectSettingsSection(page, 'federation')
    const panel = page.getByTestId('metahub-settings')
    await expect(panel).toBeVisible()
    await expect(workspace).toHaveAttribute('data-workspace', 'conversation')
    await panel.getByLabel('MetaHub address').fill('127.0.0.1:9900')
    await panel.getByLabel('MetaHub hub name').fill('desktop-e2e')
    await panel.getByLabel('MetaHub token').fill('never-render-this-again')
    await panel.getByLabel('Join MetaHub on session start').check()
    await panel.getByRole('button', { name: 'Save', exact: true }).click()
    await expect(panel.getByLabel('MetaHub token')).toHaveValue('')
    await expect(panel.getByLabel('MetaHub token')).toHaveAttribute(
      'placeholder', 'Configured · replace token',
    )

    await panel.getByRole('button', { name: 'Start local & join' }).click()
    await expect(panel).toContainText('connected · local coordinator running')
    const topologyAgent = panel.getByTestId('metahub-topology')
      .locator('[data-agent-id$="/main"]')
    await expect(topologyAgent).toHaveCount(1)
    const agentId = await topologyAgent.getAttribute('data-agent-id')
    expect(agentId).toMatch(/^desktop-e2e-.+\/main$/)
    await page.getByRole('button', { name: 'Close settings' }).click()
    await expect(workspace).toHaveAttribute('data-workspace', 'conversation')
    await expect(page.getByLabel('Message Loopal')).toBeVisible()

    await page.getByRole('button', { name: 'Federation', exact: true }).click()
    await expect(workspace).toHaveAttribute('data-workspace', 'federation')
    const federation = page.getByTestId('federation-workspace')
    await expect(federation).toContainText(agentId!)
    await expect(federation).toContainText('1 hubs · 1 Agents')
    await expect(page.locator('.session-navigator')).toHaveCount(0)
    await expect(page.getByTestId('inspector')).toHaveCount(0)
    await expect(page.getByLabel('Message Loopal')).toHaveCount(0)
    await expectFullWorkspace(page)

    await federation.getByRole('button', { name: 'Open conversation' }).click()
    await expect(workspace).toHaveAttribute('data-workspace', 'conversation')
    await expect(page.getByLabel('Message Loopal')).toBeVisible()

    await page.getByRole('button', { name: 'Federation', exact: true }).click()
    await federation.getByRole('button', { name: 'Manage federation', exact: true }).click()
    await selectSettingsSection(page, 'federation')
    await expect(workspace).toHaveAttribute('data-workspace', 'federation')
    await panel.getByRole('button', { name: 'Disconnect' }).click()
    await expect(panel).toContainText('disconnected · local coordinator running')
    await page.getByRole('button', { name: 'Close settings' }).click()
    await expect(workspace).toHaveAttribute('data-workspace', 'federation')
    await expect(federation).toContainText('Federation is running.')

    await federation.getByRole('button', { name: 'Manage federation' }).click()
    await selectSettingsSection(page, 'federation')
    await panel.getByRole('button', { name: 'Clear stored token' }).click()
    await expect(panel.getByLabel('MetaHub token')).toHaveAttribute('placeholder', 'Required')
    await panel.getByRole('button', { name: 'Stop local MetaHub' }).click()
    await expect(panel).toContainText('disconnected · local coordinator stopped')
    await page.getByRole('button', { name: 'Close settings' }).click()
    await expect(workspace).toHaveAttribute('data-workspace', 'federation')
    await expectBackground(desktop)
  } finally {
    await closeDesktop(desktop)
  }
})

async function expectFullWorkspace(page: import('@playwright/test').Page): Promise<void> {
  const rail = await page.locator('.activity-bar').boundingBox()
  const workspace = await page.getByTestId('primary-workspace').boundingBox()
  const viewport = page.viewportSize()
  expect(rail && workspace && viewport).toBeTruthy()
  expect(Math.abs(workspace!.x - rail!.x - rail!.width)).toBeLessThanOrEqual(1)
  expect(Math.abs(workspace!.x + workspace!.width - viewport!.width)).toBeLessThanOrEqual(1)
}

async function expectBackground(desktop: DesktopFixture): Promise<void> {
  expect(await desktop.app.evaluate(({ BrowserWindow }) => {
    const window = BrowserWindow.getAllWindows()[0]
    return { visible: window?.isVisible(), focused: window?.isFocused() }
  })).toEqual({ visible: false, focused: false })
}
