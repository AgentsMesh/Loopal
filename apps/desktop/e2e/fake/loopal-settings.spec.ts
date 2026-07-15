import { expect, test } from '@playwright/test'
import { closeDesktop, launchDesktop } from '../support/electron/electron-fixture'
import { selectSettingsSection } from '../support/settings/settings-helpers'

test('navigates filtered second-level settings without rendering every section', async () => {
  const desktop = await launchDesktop('fake')
  try {
    const page = desktop.page
    await page.getByRole('button', { name: 'Settings' }).click()
    const navigation = page.getByTestId('settings-navigation')
    await expect(navigation.getByRole('tab')).toHaveCount(8)
    await expect(navigation.locator('[data-section="appearance"]'))
      .toHaveAttribute('aria-selected', 'true')
    await expect(page.getByTestId('desktop-language')).toBeVisible()
    await expect(page.getByTestId('loopal-default-settings')).toHaveCount(0)
    await expect(page.getByRole('group', { name: 'Agent controls' })).toHaveCount(0)

    const search = page.getByTestId('settings-search')
    await search.fill('MCP')
    await expect(navigation.getByRole('tab')).toHaveCount(2)
    await expect(navigation.locator('[data-section="mcp"]'))
      .toHaveAttribute('aria-selected', 'true')
    await expect(page.getByTestId('loopal-mcp-settings')).toBeVisible()
    await search.fill('not-a-real-setting')
    await expect(navigation.getByRole('tab')).toHaveCount(0)
    await expect(page.getByTestId('settings-section-content').getByRole('status'))
      .toContainText('No settings sections found')
    await search.fill('')
    await expect(navigation.getByRole('tab')).toHaveCount(8)
    for (const [section, scope] of [
      ['appearance', 'Desktop · User'],
      ['loopal', 'Loopal · User · ~/.loopal/settings.json'],
      ['providers', 'Loopal · User · ~/.loopal/settings.json'],
      ['skills', 'Loopal · User Skills and current project sources'],
      ['mcp', 'MCP · Current session directory'],
      ['agent', 'Session · Current session'],
      ['runtime', 'Session · Current session'],
      ['federation', 'MetaHub · Application'],
    ] as const) {
      await selectSettingsSection(page, section)
      await expect(page.getByTestId('settings-section-content')).toContainText(scope)
    }
  } finally {
    await closeDesktop(desktop)
  }
})

test('edits persistent defaults separately from the current live Agent', async () => {
  const desktop = await launchDesktop('fake')
  try {
    await desktop.page.getByRole('button', { name: 'Settings' }).click()
    await selectSettingsSection(desktop.page, 'loopal')
    const pane = desktop.page.getByTestId('settings-pane')
    await expect(pane.getByRole('heading', {
      name: 'Loopal defaults (new/restarted Sessions)',
    })).toBeVisible()
    await selectSettingsSection(desktop.page, 'agent')
    await expect(pane.getByRole('heading', { name: 'Current Agent (live)' })).toBeVisible()
    await selectSettingsSection(desktop.page, 'providers')
    await expect(pane.getByTestId('configured-providers')).toContainText(
      'credentials hidden',
    )
    await selectSettingsSection(desktop.page, 'loopal')
    const model = pane.getByLabel('Default model')
    await expect(model).toHaveValue('claude-opus-4-8')
    await model.fill('desktop-e2e-model')
    await pane.getByLabel('Summarization override').fill('desktop-summary-model')
    await selectSettingsSection(desktop.page, 'providers')
    await pane.getByLabel('Enable Anthropic').check()
    await pane.getByLabel('Anthropic base URL').fill('https://proxy.example.test/v1')
    await pane.getByLabel('Anthropic API key environment').fill('LOOPAL_E2E_ANTHROPIC_KEY')
    await pane.getByLabel('Anthropic API key', { exact: true }).fill('write-only-e2e-value')
    const restart = pane.getByRole('button', { name: 'Restart current Session' })
    await expect(restart).toBeDisabled()
    await pane.getByRole('button', { name: 'Save provider settings' }).click()
    await expect(pane.getByRole('status')).toContainText('new or restarted Sessions')
    await expect(pane.getByLabel('Anthropic API key', { exact: true })).toHaveValue('')
    await expect(pane.getByLabel('Anthropic API key', { exact: true }))
      .toHaveAttribute('placeholder', 'Configured')
    await selectSettingsSection(desktop.page, 'loopal')
    await pane.getByText('Advanced resolved config').click()
    await pane.getByLabel('Search resolved config').fill('model_routing.summarization')
    await expect(pane.getByRole('cell', { name: 'desktop-summary-model' })).toBeVisible()
    await expect(restart).toBeEnabled()
    await restart.click()
    await expect(pane.getByRole('status')).toContainText('restarted with the saved')
    await desktop.page.getByRole('button', { name: 'Close settings' }).click()
    await desktop.page.getByRole('button', { name: 'Settings' }).click()
    await selectSettingsSection(desktop.page, 'loopal')
    await expect(desktop.page.getByLabel('Default model')).toHaveValue('desktop-e2e-model')
    await expect(desktop.page.getByLabel('Summarization override'))
      .toHaveValue('desktop-summary-model')
    await selectSettingsSection(desktop.page, 'providers')
    await expect(desktop.page.getByLabel('Enable Anthropic')).toBeChecked()
  } finally {
    await closeDesktop(desktop)
  }
})
