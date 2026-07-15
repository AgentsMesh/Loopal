import { expect, test } from '@playwright/test'
import { closeDesktop, launchDesktop } from '../support/electron/electron-fixture'

test('denies popup, navigation, and permission requests at runtime', async () => {
  const desktop = await launchDesktop('fake')
  try {
    await desktop.page.evaluate(() => {
      const popup = document.createElement('button')
      popup.id = 'security-popup'
      popup.style.cssText = 'position:fixed;z-index:2147483647;inset:8px auto auto 8px'
      popup.addEventListener('click', () => {
        popup.dataset.result = window.open('http://loopal.invalid/popup') === null
          ? 'denied'
          : 'opened'
      })
      const permission = document.createElement('button')
      permission.id = 'security-permission'
      permission.style.cssText = 'position:fixed;z-index:2147483647;inset:40px auto auto 8px'
      permission.addEventListener('click', async () => {
        permission.dataset.result = await Notification.requestPermission()
      })
      document.body.append(popup, permission)
    })

    await desktop.page.locator('#security-popup').click()
    await expect(desktop.page.locator('#security-popup')).toHaveAttribute(
      'data-result',
      'denied',
    )
    await expect.poll(() => desktop.app.windows().length).toBe(1)

    await desktop.page.locator('#security-permission').click()
    await expect(desktop.page.locator('#security-permission')).toHaveAttribute(
      'data-result',
      'denied',
    )

    const target = 'https://loopal.invalid/navigation'
    const original = desktop.page.url()
    await desktop.app.evaluate(({ BrowserWindow }) => {
      const contents = BrowserWindow.getAllWindows()[0]?.webContents
      if (!contents) return
      Reflect.set(contents, '__loopalNavigationAttempt', '')
      contents?.once('will-navigate', (_event, url) => {
        Reflect.set(contents, '__loopalNavigationAttempt', url)
      })
    })
    await desktop.page.evaluate((url) => {
      const link = document.createElement('a')
      link.href = url
      link.id = 'security-navigation'
      link.textContent = 'navigate'
      link.style.cssText = 'position:fixed;z-index:2147483647;inset:72px auto auto 8px'
      document.body.append(link)
    }, target)
    await desktop.page.locator('#security-navigation').click({ noWaitAfter: true })
    await expect.poll(() => desktop.app.evaluate(({ BrowserWindow }) => {
      const contents = BrowserWindow.getAllWindows()[0]?.webContents
      return contents ? Reflect.get(contents, '__loopalNavigationAttempt') : undefined
    })).toBe(target)
    expect(desktop.page.url()).toBe(original)
  } finally {
    await closeDesktop(desktop)
  }
})
