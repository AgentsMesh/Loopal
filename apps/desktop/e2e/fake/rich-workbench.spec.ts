import { expect, test } from '@playwright/test'
import { closeDesktop, launchDesktop } from '../support/electron/electron-fixture'
import { selectSettingsSection } from '../support/settings/settings-helpers'

test('renders Loopal rich conversation, runtime, task, and diagnostics state', async () => {
  const desktop = await launchDesktop('fake')
  try {
    await expect(desktop.page.getByTestId('active-session-title')).toHaveText(
      'Build LoopalDesktop foundation',
    )
    const conversation = desktop.page.getByTestId('conversation')
    await expect(conversation.getByRole('heading', { name: 'Verified desktop state' })).toBeVisible()
    await expect(conversation.getByText('Bazel-only')).toBeVisible()
    await expect(conversation.locator('pre[data-language="ts"]')).toContainText(
      'const desktop = ready',
    )
    await expect(conversation.getByText('1 image attachment(s)')).toBeVisible()
    await expect(conversation.getByLabel('Streaming')).toBeVisible()
    await expect(conversation.getByText('Retrying one transient provider response.')).toBeVisible()
    await expect(conversation.getByText('Summarizing context for the next turn.')).toBeVisible()
    await expect(conversation.getByText(
      'Session resume warning: one scheduled job needs review.',
    )).toBeVisible()
    await expect(desktop.page.getByTestId('inspector')).toHaveCount(0)

    const tool = conversation.getByTestId('tool-invocation')
    await expect(tool.getByLabel('Completed')).toBeVisible()
    await tool.locator(':scope > summary').click()
    await expect(tool.getByText('Build completed')).toBeVisible()
    await tool.getByText('Input', { exact: true }).click()
    await expect(tool.getByText(/bazel build \/\/apps\/desktop:out/)).toBeVisible()

    const runtime = desktop.page.getByTestId('runtime-status')
    await expect(runtime).toContainText('Thinking')
    await expect(runtime).toContainText('Running Electron verification')
    await expect(runtime).toContainText('Context 1%')

    await activatePanel(desktop.page, 'Agents')
    const topology = desktop.page.getByTestId('agents-pane')
    await expect(topology.getByRole('treeitem')).toHaveCount(2)
    await topology.locator('[data-agent-id="agent-e2e"]').click()
    await expect(conversation).toContainText('E2E specialist verified the Electron renderer.')
    await topology.locator('[data-agent-id="agent-root"]').click()
    await expect(conversation).toContainText(
      'Session resume warning: one scheduled job needs review.',
    )

    await activatePanel(desktop.page, 'Tasks')
    const tasks = desktop.page.getByTestId('tasks-pane')
    await expect(tasks).toContainText('Ship a verified Loopal Desktop')
    await expect(tasks).toContainText('Plan · 0/2')
    await activatePanel(desktop.page, 'Background')
    await expect(desktop.page.getByTestId('background-tasks-pane')).toContainText(
      'Bazel test runner',
    )
    await activatePanel(desktop.page, 'Scheduled')
    await expect(desktop.page.getByTestId('scheduled-work-pane')).toContainText(
      'Check Desktop Host health',
    )

    await desktop.page.getByRole('button', { name: 'Settings' }).click()
    await selectSettingsSection(desktop.page, 'runtime')
    const diagnostics = desktop.page.getByTestId('settings-pane')
      .getByTestId('diagnostics-pane')
    await expect(diagnostics).toContainText('Runtime configuration')
    await expect(diagnostics).toContainText('Usage')
    await expect(diagnostics).toContainText('filesystem')
    await expect(diagnostics).toContainText('1 active · 4 total')

    await desktop.page.getByRole('button', { name: 'Close settings' }).click()
    await desktop.page.locator('[data-session-id="session-protocol"]').click()
    await expect(desktop.page.getByTestId('session-panel-zone')).toHaveCount(0)
  } finally {
    await closeDesktop(desktop)
  }
})

async function activatePanel(page: import('@playwright/test').Page, name: string): Promise<void> {
  const tab = page.getByRole('tab', { name, exact: true })
  await tab.click()
  await expect(tab).toHaveAttribute('aria-selected', 'true')
}
