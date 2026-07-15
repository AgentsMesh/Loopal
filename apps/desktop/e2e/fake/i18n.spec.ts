import { expect, test } from '@playwright/test'
import {
  closeDesktop, launchDesktop, relaunchDesktop, type DesktopFixture,
} from '../support/electron/electron-fixture'
import { selectSettingsSection } from '../support/settings/settings-helpers'

test('switches Desktop language immediately and persists it across relaunch', async () => {
  let desktop = await launchDesktop('fake')
  try {
    await expectLocale(desktop, 'en', 'system')
    await expect(desktop.page.locator('.navigator-header h1')).toHaveText('Sessions')
    await expect(desktop.page.getByLabel('Search sessions')).toBeVisible()
    await expect(desktop.page.getByTestId('current-session-list')).toContainText(
      'Current sessions',
    )
    await expect(desktop.page.getByTestId('history-session-list')).toHaveCount(0)

    await desktop.page.getByRole('button', { name: 'Settings' }).click()
    const language = desktop.page.getByTestId('desktop-language')
    await expect(language).toBeEnabled()
    await expect(language).toHaveValue('system')
    await language.selectOption('zh-CN')

    await expectLocale(desktop, 'zh-CN', 'zh-CN')
    await expect(desktop.page.getByTestId('settings-pane')).toContainText('桌面外观')
    const chineseNavigation = desktop.page.getByTestId('settings-navigation')
    await expect(chineseNavigation.getByRole('tab')).toHaveCount(8)
    for (const label of [
      '桌面外观', '默认值', '模型提供商', 'Skills 与 Plugins', 'MCP 服务器',
      '当前 Agent（实时）', '运行时与 MCP', 'MetaHub',
    ]) await expect(chineseNavigation.getByRole('tab', { name: label })).toBeVisible()
    await expect(desktop.page.getByTestId('settings-section-content'))
      .toContainText('桌面 · 用户')
    await selectSettingsSection(desktop.page, 'loopal')
    await expect(desktop.page.getByTestId('loopal-default-settings'))
      .toContainText('Loopal 默认设置（新建/重启的会话）')
    await expect(desktop.page.getByTestId('settings-section-content'))
      .toContainText('Loopal · 用户 · ~/.loopal/settings.json')
    await expect(desktop.page.getByText('终端字号', { exact: true })).toHaveCount(0)
    await expect(desktop.page.getByText('终端回滚行数', { exact: true })).toHaveCount(0)
    await expect(desktop.page.locator('.navigator-header h1')).toHaveText('会话')
    await desktop.page.getByRole('button', { name: '关闭设置' }).click()
    await expect(desktop.page.getByLabel('搜索会话')).toBeVisible()
    await expect(desktop.page.getByTestId('current-session-list')).toContainText('当前会话')
    await expect(desktop.page.getByTestId('history-session-list')).toHaveCount(0)
    await desktop.page.getByRole('button', { name: '联邦', exact: true }).click()
    await expect(desktop.page.getByTestId('federation-workspace')).toContainText(
      '为 Loopal 会话启动联邦。',
    )
    await desktop.page.getByRole('button', { name: '对话', exact: true }).click()
    await expect(desktop.page.getByRole('tab', { name: '任务', exact: true })).toBeVisible()
    await desktop.page.getByRole('tab', { name: 'Agent', exact: true }).click()
    await expect(desktop.page.getByTestId('agents-pane')).toBeVisible()
    await expect(desktop.page.getByRole('button', { name: '终端', exact: true })).toHaveCount(0)
    await expect(desktop.page.getByTestId('terminal-panel')).toHaveCount(0)

    desktop = await relaunchDesktop(desktop)
    await expectLocale(desktop, 'zh-CN', 'zh-CN')
    await expect(desktop.page.locator('.navigator-header h1')).toHaveText('会话')
    await desktop.page.getByRole('button', { name: '设置' }).click()
    await selectSettingsSection(desktop.page, 'appearance')
    await expect(desktop.page.getByTestId('desktop-language')).toHaveValue('zh-CN')
    await desktop.page.getByTestId('desktop-language').selectOption('en')

    await expectLocale(desktop, 'en', 'en')
    await expect(desktop.page.getByTestId('settings-pane')).toContainText('Desktop appearance')
    const englishNavigation = desktop.page.getByTestId('settings-navigation')
    await expect(englishNavigation.getByRole('tab')).toHaveCount(8)
    for (const label of [
      'Desktop appearance', 'Defaults', 'Model providers', 'Skills & Plugins', 'MCP servers',
      'Current Agent (live)', 'Runtime and MCP', 'MetaHub',
    ]) await expect(englishNavigation.getByRole('tab', { name: label })).toBeVisible()
    await expect(desktop.page.getByTestId('settings-section-content'))
      .toContainText('Desktop · User')
    await expect(desktop.page.getByText('Terminal font size', { exact: true })).toHaveCount(0)
    await expect(desktop.page.getByText('Terminal scrollback', { exact: true })).toHaveCount(0)
    await expect(desktop.page.locator('.navigator-header h1')).toHaveText('Sessions')
    await expect(desktop.page.getByRole('button', { name: 'Terminal', exact: true })).toHaveCount(0)
    await desktop.page.getByRole('button', { name: 'Close settings' }).click()

    desktop = await relaunchDesktop(desktop)
    await expectLocale(desktop, 'en', 'en')
    await expect(desktop.page.getByRole('button', { name: 'Settings' })).toBeAttached()
  } finally {
    await closeDesktop(desktop)
  }
})

async function expectLocale(
  desktop: DesktopFixture,
  documentLocale: 'en' | 'zh-CN',
  preference: 'system' | 'en' | 'zh-CN',
): Promise<void> {
  await expect(desktop.page.locator('html')).toHaveAttribute('lang', documentLocale)
  await expect.poll(() => desktop.page.evaluate(
    () => window.loopalDesktop.getDesktopPreferences(),
  )).toEqual({ locale: preference })
  const windowState = await desktop.app.evaluate(({ BrowserWindow }) => {
    const current = BrowserWindow.getAllWindows()[0]
    return { visible: current?.isVisible(), focused: current?.isFocused() }
  })
  expect(windowState).toEqual({ visible: false, focused: false })
}
