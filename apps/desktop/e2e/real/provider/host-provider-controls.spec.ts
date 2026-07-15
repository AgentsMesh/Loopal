import { expect, test, type Page } from '@playwright/test'
import { closeDesktop, launchDesktop } from '../../support/electron/electron-fixture'
import { ready, send } from '../../support/runtime/llm-e2e-helpers'
import { selectSettingsSection } from '../../support/settings/settings-helpers'

test('applies model, thinking, mode, and clear controls to real provider requests', async () => {
  const desktop = await launchDesktop('real', 'provider-controls')
  try {
    const page = desktop.page
    const conversation = page.getByTestId('conversation')
    await ready(page)
    await send(page, 'Seed control history')
    await expect(conversation).toContainText('CONTROL BASELINE MARKER', { timeout: 20_000 })
    await ready(page)

    await openControls(page)
    const controls = page.getByRole('group', { name: 'Agent controls' })
    await controls.getByRole('textbox', { name: 'Agent model' }).fill('claude-sonnet-4-6')
    await controls.getByRole('button', { name: 'Apply agent model' }).click()
    await expect(controls.getByRole('textbox', { name: 'Agent model' }))
      .toHaveValue('claude-sonnet-4-6')
    await expect(controls.getByLabel('Thinking configuration')).toBeEnabled()
    await controls.getByLabel('Thinking configuration').selectOption('high')
    await expect(controls.getByLabel('Thinking configuration')).toHaveValue('high')
    const agentMode = controls.getByLabel('Agent mode', { exact: true })
    await expect(agentMode).toBeEnabled()
    await agentMode.selectOption('plan')
    await expect(agentMode).toHaveValue('plan')
    await closeControls(page)

    await send(page, 'Observe high thinking controls')
    await expect(conversation).toContainText('CONTROLLED THINKING STREAM', { timeout: 20_000 })
    await expect(conversation).toContainText(
      'High thinking and model switch reached the provider.', { timeout: 20_000 },
    )
    await ready(page)

    await openControls(page)
    await controls.getByLabel('Thinking configuration').selectOption('disabled')
    await expect(controls.getByLabel('Thinking configuration')).toHaveValue('disabled')
    await agentMode.selectOption('act')
    await expect(agentMode).toHaveValue('act')
    await closeControls(page)
    await send(page, 'Observe disabled thinking controls')
    await expect(conversation).toContainText(
      'Disabled thinking reached the provider.', { timeout: 20_000 },
    )
    await ready(page)

    await openControls(page)
    await controls.getByRole('button', { name: 'Clear' }).click()
    await expect(conversation.locator('[data-message-role]:not([data-message-role="system"])'))
      .toHaveCount(0)
    await closeControls(page)
    await send(page, 'Verify clear request history')
    await expect(conversation).toContainText('Clear removed prior model history.', {
      timeout: 20_000,
    })
    await ready(page)

    await send(page, 'Seed rewind second turn')
    await expect(conversation).toContainText('REWIND SECOND MARKER', { timeout: 20_000 })
    await ready(page)
    await openControls(page)
    await controls.getByLabel('Rewind turn index').fill('0')
    await controls.getByRole('button', { name: 'Rewind', exact: true }).click()
    await expect(conversation.locator('[data-message-role]:not([data-message-role="system"])'))
      .toHaveCount(0)
    await closeControls(page)
    await send(page, 'Verify rewind request history')
    await expect(conversation).toContainText('Rewind removed later model history.', {
      timeout: 20_000,
    })
    await ready(page)

    const requests = await desktop.llm!.requests()
    expect(requests).toHaveLength(6)
    expect(requests[1]).toMatchObject({ model: 'claude-sonnet-4-6', thinkingEnabled: true })
    expect(requests[2]).toMatchObject({ model: 'claude-sonnet-4-6', thinkingEnabled: false })
    expect(requests[3]).toMatchObject({ model: 'claude-sonnet-4-6', messageCount: 1 })
    expect(requests[5]).toMatchObject({ model: 'claude-sonnet-4-6', messageCount: 1 })
    await expect.poll(() => desktop.llm!.state()).toMatchObject({
      served: 6, remaining: 0, verified: true,
    })
  } finally {
    await closeDesktop(desktop)
  }
})

async function openControls(page: Page): Promise<void> {
  await page.getByRole('button', { name: 'Settings' }).click()
  await selectSettingsSection(page, 'agent')
  await expect(page.getByRole('group', { name: 'Agent controls' })).toBeVisible()
}

async function closeControls(page: Page): Promise<void> {
  await page.getByRole('button', { name: 'Close settings' }).click()
}
