import { expect, test, type Page } from '@playwright/test'
import {
  closeDesktop, launchDesktop, type DesktopFixture,
} from '../support/electron/electron-fixture'

const viewports = [
  { width: 1440, height: 900 },
  { width: 900, height: 620 },
] as const
const shortcutModifier = process.platform === 'darwin' ? 'Meta' : 'Control'

for (const viewport of viewports) {
  test(`manages Conversation and Federation spaces at ${viewport.width}x${viewport.height}`, async () => {
    const desktop = await launchDesktop('fake')
    try {
      const page = desktop.page
      await page.setViewportSize(viewport)
      await expectBackground(desktop)
      const workspace = page.getByTestId('primary-workspace')
      const conversationButton = page.getByRole('button', { name: 'Conversation', exact: true })
      const federationButton = page.getByRole('button', { name: 'Federation', exact: true })
      await expect(conversationButton).toHaveAttribute('aria-pressed', 'true')
      await expect(federationButton).toHaveAttribute('aria-pressed', 'false')
      await expect(workspace).toHaveAttribute('data-workspace', 'conversation')
      await expect(page.getByTestId('inspector')).toHaveCount(0)
      await expect(page.getByLabel('Message Loopal')).toBeInViewport()

      const expanded = await workspace.boundingBox()
      await page.getByRole('button', { name: 'Toggle sidebar' }).click()
      await expect(page.locator('.session-navigator')).toHaveCount(0)
      const collapsed = await workspace.boundingBox()
      expect(expanded && collapsed && collapsed.width > expanded.width).toBe(true)
      await page.getByRole('button', { name: 'Toggle sidebar' }).click()
      await expect(page.locator('.session-navigator')).toBeVisible()

      await federationButton.click()
      await expect(federationButton).toHaveAttribute('aria-pressed', 'true')
      await expect(workspace).toHaveAttribute('data-workspace', 'federation')
      await expect(page.getByTestId('federation-workspace')).toContainText(
        'Start a Federation for your Loopal sessions.',
      )
      await expect(page.locator('.session-navigator')).toHaveCount(0)
      await expect(page.getByLabel('Message Loopal')).toHaveCount(0)
      await expectFullWorkspace(page)

      const settingsButton = page.getByRole('button', { name: 'Settings' })
      await settingsButton.click()
      await expect(page.getByTestId('settings-pane')).toBeVisible()
      await expect(workspace).toHaveAttribute('data-workspace', 'federation')
      await expect(page.locator('.settings-overlay')).toHaveAttribute('data-workspace', 'settings')
      await expect(page.getByRole('button', { name: 'Close settings' })).toBeFocused()
      await page.keyboard.press('Escape')
      await expect(page.getByTestId('settings-pane')).toHaveCount(0)
      await expect(settingsButton).toBeFocused()
      await expect(workspace).toHaveAttribute('data-workspace', 'federation')
      await expect(page.getByTestId('federation-workspace')).toBeVisible()

      await conversationButton.click()
      await expect(workspace).toHaveAttribute('data-workspace', 'conversation')
      await expect(page.locator('.session-navigator')).toBeVisible()
      await expect(page.getByLabel('Message Loopal')).toBeInViewport()
      await page.keyboard.press(`${shortcutModifier}+2`)
      await expect(workspace).toHaveAttribute('data-workspace', 'federation')
      await page.keyboard.press(`${shortcutModifier}+1`)
      await expect(workspace).toHaveAttribute('data-workspace', 'conversation')
      await expectBackground(desktop)
    } finally {
      await closeDesktop(desktop)
    }
  })
}

async function expectFullWorkspace(page: Page): Promise<void> {
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
