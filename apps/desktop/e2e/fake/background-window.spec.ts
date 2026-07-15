import { expect, test } from '@playwright/test'
import {
  closeDesktop, launchDesktop, relaunchDesktop, type DesktopFixture,
} from '../support/electron/electron-fixture'

test('keeps the Electron E2E window hidden and unfocused across relaunch', async () => {
  let desktop = await launchDesktop('fake')
  try {
    await expect(desktop.page.getByTestId('workbench')).toBeAttached()
    await expectHidden(desktop)
    await desktop.page.getByRole('button', { name: 'Settings' }).click()
    await expect(desktop.page.getByTestId('settings-pane')).toBeAttached()
    desktop = await relaunchDesktop(desktop)
    await expectHidden(desktop)
  } finally {
    await closeDesktop(desktop)
  }
})

test('exposes draggable title surfaces without swallowing controls', async () => {
  test.skip(process.platform !== 'darwin', 'native title bar is used outside macOS')
  const desktop = await launchDesktop('fake')
  try {
    await expectHidden(desktop)
    await expect(desktop.page.locator('html')).toHaveAttribute('data-platform', 'darwin')
    await expectHitRegion(desktop, '.activity-bar', 'drag')
    await expectHitRegion(desktop, '.navigator-header', 'drag')
    await expectHitRegion(desktop, '.session-toolbar', 'drag')
    await expectHitRegion(desktop, '.new-session', 'no-drag')
    await expect(desktop.page.locator('.context-switcher')).toHaveCount(0)
    await expectHitRegion(desktop, '.toolbar-actions button:last-child', 'no-drag')
    await desktop.page.getByRole('button', { name: 'Session details' }).click()
    await expect(desktop.page.getByTestId('session-metadata')).toBeVisible()
    await desktop.page.getByRole('button', { name: 'Settings' }).click()
    await expect(desktop.page.getByTestId('settings-pane')).toBeAttached()
    await expectHitRegion(desktop, '.settings-header', 'drag')
    await expectHitRegion(desktop, '.settings-header button', 'no-drag')
    await expectHidden(desktop)
  } finally {
    await closeDesktop(desktop)
  }
})

async function expectHidden(desktop: DesktopFixture): Promise<void> {
  const state = await desktop.app.evaluate(({ BrowserWindow }) => {
    const window = BrowserWindow.getAllWindows()[0]
    return { visible: window?.isVisible(), focused: window?.isFocused() }
  })
  expect(state).toEqual({ visible: false, focused: false })
}

async function expectHitRegion(
  desktop: DesktopFixture,
  selector: string,
  expected: 'drag' | 'no-drag',
): Promise<void> {
  await expect.poll(() => desktop.page.locator(selector).evaluate((element) => {
    const bounds = element.getBoundingClientRect()
    let hit = document.elementFromPoint(
      bounds.left + bounds.width / 2,
      bounds.top + bounds.height / 2,
    )
    while (hit) {
      const region = getComputedStyle(hit).getPropertyValue('-webkit-app-region')
      if (region) return region
      hit = hit.parentElement
    }
    return ''
  })).toBe(expected)
}
