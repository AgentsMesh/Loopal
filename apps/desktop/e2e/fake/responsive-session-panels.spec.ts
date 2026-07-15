import { expect, test, type Page } from '@playwright/test'
import {
  closeDesktop, launchDesktop, type DesktopFixture,
} from '../support/electron/electron-fixture'

const viewports = [
  { width: 1440, height: 900 },
  { width: 900, height: 620 },
] as const

for (const viewport of viewports) {
  test(`keeps conversation primary with a compact Context Dock at ${viewport.width}x${viewport.height}`, async () => {
    const desktop = await launchDesktop('fake')
    try {
      const page = desktop.page
      await page.setViewportSize(viewport)
      await expectBackground(desktop)
      await expect(page.getByTestId('primary-workspace'))
        .toHaveAttribute('data-workspace', 'conversation')
      await expect(page.getByTestId('inspector')).toHaveCount(0)

      const conversation = page.getByTestId('conversation')
      const composer = page.getByRole('combobox', { name: 'Message Loopal' })
      const dock = page.getByTestId('session-panel-zone')
      await expect(conversation).toBeVisible()
      await expect(composer).toBeVisible()
      await expect(composer).toBeInViewport()
      await expect(dock.locator('[role="tabpanel"]:visible')).toHaveCount(0)
      await expect(dock.getByRole('separator')).toHaveCount(0)
      const collapsed = await expectGeometry(page, viewport)

      const tabs = dock.getByRole('tab')
      expect(new Set(await tabs.evaluateAll((items) => items.map(
        (item) => Math.round(item.getBoundingClientRect().y),
      ))).size).toBe(1)

      const first = tabs.first()
      await first.click()
      await expect(first).toHaveAttribute('aria-selected', 'true')
      await expect(dock.locator('[role="tabpanel"]:visible')).toHaveCount(1)
      await expectFloatingPanel(page, collapsed, viewport)
      await expect(composer).toBeInViewport()

      await first.click()
      await expect(dock.locator('[role="tabpanel"]:visible')).toHaveCount(0)
      await expectGeometry(page, viewport)
      await expectBackground(desktop)
    } finally {
      await closeDesktop(desktop)
    }
  })
}

async function expectGeometry(
  page: Page,
  viewport: { readonly width: number; readonly height: number },
): Promise<LayoutGeometry> {
  const workspace = await page.getByTestId('primary-workspace').boundingBox()
  const conversation = await page.getByTestId('conversation').boundingBox()
  const dock = await page.getByTestId('session-panel-zone').boundingBox()
  const composer = await page.getByRole('combobox', { name: 'Message Loopal' }).boundingBox()
  expect(workspace && conversation && dock && composer).toBeTruthy()
  expect(Math.abs(workspace!.x + workspace!.width - viewport.width)).toBeLessThanOrEqual(1)
  expect(conversation!.height).toBeGreaterThan(dock!.height)
  expect(composer!.y + composer!.height).toBeLessThanOrEqual(viewport.height)
  return { workspace: workspace!, conversation: conversation!, dock: dock!, composer: composer! }
}

async function expectFloatingPanel(
  page: Page, collapsed: LayoutGeometry,
  viewport: { readonly width: number; readonly height: number },
): Promise<void> {
  const panel = await page.locator('[role="tabpanel"]:visible').boundingBox()
  const expanded = await expectGeometry(page, viewport)
  expect(panel).toBeTruthy()
  expectClose(expanded.conversation.y, collapsed.conversation.y)
  expectClose(expanded.conversation.height, collapsed.conversation.height)
  expectClose(expanded.composer.y, collapsed.composer.y)
  expectClose(expanded.dock.y, collapsed.dock.y)
  expect(panel!.y).toBeLessThan(collapsed.conversation.y + collapsed.conversation.height)
  expectClose(panel!.y + panel!.height, collapsed.dock.y)
  expect(panel!.y + panel!.height).toBeLessThanOrEqual(collapsed.composer.y)
}

interface LayoutGeometry {
  readonly workspace: Box
  readonly conversation: Box
  readonly dock: Box
  readonly composer: Box
}

interface Box { readonly x: number; readonly y: number; readonly width: number; readonly height: number }

function expectClose(actual: number, expected: number): void {
  expect(Math.abs(actual - expected)).toBeLessThanOrEqual(1)
}

async function expectBackground(desktop: DesktopFixture): Promise<void> {
  expect(await desktop.app.evaluate(({ BrowserWindow }) => {
    const window = BrowserWindow.getAllWindows()[0]
    return { visible: window?.isVisible(), focused: window?.isFocused() }
  })).toEqual({ visible: false, focused: false })
}
