import { expect, test } from '@playwright/test'
import { closeDesktop, launchDesktop } from '../support/electron/electron-fixture'
import { selectSettingsSection } from '../support/settings/settings-helpers'

test('controls the exact fake agent generation through renderer actions', async () => {
  const desktop = await launchDesktop('fake')
  try {
    const mode = desktop.page.getByRole('combobox', { name: 'Agent mode' })
    await expect(mode).toHaveValue('act')
    await mode.selectOption('plan')
    await expect(mode).toHaveValue('plan')

    await desktop.page.getByRole('button', { name: 'Settings' }).click()
    await selectSettingsSection(desktop.page, 'agent')
    const actions = desktop.page.getByRole('group', { name: 'Agent controls' })
    const model = actions.getByRole('textbox', { name: 'Agent model', exact: true })
    await model.fill('gpt-5.1')
    await actions.getByRole('button', { name: 'Apply agent model' }).click()
    await expect(model).toHaveValue('gpt-5.1')
    await actions.getByLabel('Thinking configuration').selectOption('high')
    await expect(actions.getByLabel('Thinking configuration')).toHaveValue('high')
    await actions.getByLabel('Permission mode').selectOption('bypass')
    await expect(actions.getByLabel('Permission mode')).toHaveValue('bypass')
    await actions.getByLabel('Decision mode').selectOption('manual')
    await expect(actions.getByLabel('Decision mode')).toHaveValue('manual')
    await actions.getByLabel('Sandbox policy').selectOption('read_only')
    await expect(actions.getByLabel('Sandbox policy')).toHaveValue('read_only')
    await actions.getByLabel('Compact instructions').fill('Keep verified tool results')
    await actions.getByRole('button', { name: 'Compact' }).click()
    await desktop.page.getByRole('button', { name: 'Close settings' }).click()
    await expect(desktop.page.getByTestId('conversation')).toContainText(
      'Summarizing conversation context: Keep verified tool results',
    )
    await desktop.page.getByRole('button', { name: 'Settings' }).click()
    await selectSettingsSection(desktop.page, 'agent')
    await actions.getByLabel('Rewind turn index').fill('1')
    await actions.getByRole('button', { name: 'Rewind' }).click()

    await actions.getByRole('button', { name: 'Interrupt' }).click()
    await expect(actions).toContainText('waiting')
    await actions.getByRole('button', { name: 'Suspend' }).click()
    await expect(actions).toContainText('suspended')
    await actions.getByRole('button', { name: 'Unsuspend' }).click()
    await expect(actions).toContainText('waiting')

    await actions.getByRole('button', { name: 'Clear' }).click()
    await desktop.page.getByRole('button', { name: 'Close settings' }).click()
    await expect(desktop.page.getByTestId('conversation')).not.toContainText(
      'Build a durable desktop workbench around Loopal.',
    )

    await activatePanel(desktop.page, 'Tasks')
    const tasks = desktop.page.getByTestId('tasks-pane')
    await expect(tasks).toContainText('Current objective')
    await expect(tasks).toContainText('Reported by the Loopal runtime')
    await expect(tasks.getByRole('button')).toHaveCount(0)
    await expect(tasks.getByRole('textbox')).toHaveCount(0)
    await activatePanel(desktop.page, 'Background')
    const background = desktop.page.getByTestId('background-tasks-pane')
    await background.getByRole('button', { name: /Kill background task/ }).click()
    await expect(desktop.page.getByRole('tab', { name: 'Background' })).toHaveCount(0)
    await activatePanel(desktop.page, 'Scheduled')
    const scheduled = desktop.page.getByTestId('scheduled-work-pane')
    await scheduled.getByRole('button', { name: /Delete scheduled work/ }).click()
    await expect(desktop.page.getByRole('tab', { name: 'Scheduled' })).toHaveCount(0)

    await activatePanel(desktop.page, 'MCP')
    const mcp = desktop.page.getByTestId('mcp-runtime-pane')
    await mcp.getByRole('button', { name: 'Refresh MCP status' }).click()
    await mcp.getByText('filesystem').click()
    await mcp.getByRole('button', { name: 'Disconnect MCP server filesystem' }).click()
    await expect(mcp).toContainText('disconnected')
    await mcp.getByRole('button', { name: 'Reconnect MCP server filesystem' }).click()
    await expect(mcp).toContainText('ready')

    const search = desktop.page.getByLabel('Search sessions')
    await search.fill('audit reference')
    await desktop.page.getByTestId('history-session-list')
      .locator('[data-session-id="session-audit"]').click()
    await search.fill('')
    await expect(desktop.page.getByTestId('active-session-title')).toContainText(
      'Audit reference applications',
    )
    await desktop.page.getByRole('button', { name: 'Settings' }).click()
    await selectSettingsSection(desktop.page, 'agent')
    await expect(desktop.page.getByRole('button', { name: 'Interrupt' })).toBeDisabled()

    const staleError = await desktop.page.evaluate(async () => {
      const bootstrap = await window.loopalDesktop.bootstrap()
      const runtime = bootstrap.runtimes.find((item) => item.sessionId === 'session-desktop')!
      try {
        await window.loopalDesktop.interruptAgent({
          sessionId: runtime.sessionId, runtimeId: runtime.id,
          generation: runtime.generation + 1, agentId: runtime.rootAgent,
        })
        return ''
      } catch (error) {
        return error instanceof Error ? error.message : String(error)
      }
    })
    expect(staleError).toContain('Session runtime is gone')
  } finally {
    await closeDesktop(desktop)
  }
})

async function activatePanel(page: import('@playwright/test').Page, name: string): Promise<void> {
  const tab = page.getByRole('tab', { name, exact: true })
  await tab.click()
  await expect(tab).toHaveAttribute('aria-selected', 'true')
}
