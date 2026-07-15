import { expect, test } from '@playwright/test'
import {
  closeDesktop, launchDesktop, waitForHostStatus,
} from '../support/electron/electron-fixture'
import { selectSettingsSection } from '../support/settings/settings-helpers'

test('boots a sandboxed workbench through the explicit preload API', async () => {
  const desktop = await launchDesktop('fake')
  try {
    await waitForHostStatus(desktop.page, 'ready')
    const surface = await desktop.page.evaluate(() => ({
      api: Object.keys(window.loopalDesktop).sort(),
      hasProcess: 'process' in window,
      hasRequire: 'require' in window,
    }))
    expect(surface).toEqual({
      api: [
        'bootstrap', 'controlAgent', 'createSession', 'createWorktree',
        'deleteGlobalSkill', 'deleteMcpServer', 'disconnectMetaHub',
        'getDesktopPreferences', 'getLocalMetaHubStatus', 'getLoopalSettings',
        'getMetaHubSettings', 'getMetaHubStatus', 'getSkill', 'gitDiff',
        'gitStage', 'gitStatus', 'gitUnstage', 'interruptAgent', 'joinMetaHub',
        'listDirectory', 'listMcpServers', 'listPlugins', 'listSkills', 'listWorktrees', 'onEvent',
        'openSession', 'readFile', 'removeWorktree', 'respondPermission', 'respondPlanApproval', 'respondQuestion', 'restartSession',
        'searchWorkspace', 'selectImages', 'selectSessionDirectory', 'sendMessage',
        'startLocalMetaHub', 'stopLocalMetaHub', 'stopSession',
        'updateDesktopPreferences', 'updateLoopalSettings', 'updateMetaHubSettings',
        'upsertGlobalSkill', 'upsertMcpServer', 'writeFile',
      ],
      hasProcess: false,
      hasRequire: false,
    })
  } finally {
    await closeDesktop(desktop)
  }
})

test('filters sessions and operates every central session panel', async () => {
  const desktop = await launchDesktop('fake')
  try {
    await expect(desktop.page.getByTestId('active-session-title')).toHaveText(
      'Build LoopalDesktop foundation',
    )
    const current = desktop.page.getByTestId('current-session-list')
    await expect(current.locator('.session-card')).toHaveCount(2)
    await expect(current).toContainText('Build LoopalDesktop foundation')
    await expect(current).toContainText('Version Desktop Control Protocol')
    await expect(current).not.toContainText('Audit reference applications')
    await expect(desktop.page.getByTestId('history-session-list')).toHaveCount(0)
    await expect(desktop.page.getByLabel('Active workspace')).toHaveCount(0)
    await expect(desktop.page.getByLabel('Active session')).toHaveCount(0)
    const search = desktop.page.getByLabel('Search sessions')
    await search.fill('audit reference')
    await expect(desktop.page.getByTestId('current-session-list')).toHaveCount(0)
    const history = desktop.page.getByTestId('history-session-list')
    await expect(history).toContainText('Audit reference applications')
    await history.locator('[data-session-id="session-audit"]').click()
    await expect(desktop.page.getByTestId('active-session-title')).toHaveText(
      'Audit reference applications',
    )
    await search.fill('protocol')
    await expect(desktop.page.getByTestId('history-session-list')).toHaveCount(0)
    await expect(desktop.page.getByTestId('current-session-list')).toContainText(
      'Version Desktop Control Protocol',
    )
    await expect(desktop.page.getByTestId('session-list')).not.toContainText(
      'Build LoopalDesktop foundation',
    )
    await search.fill('')
    await current.locator('[data-session-id="session-protocol"]').click()
    await expect(desktop.page.getByTestId('active-session-title')).toHaveText(
      'Version Desktop Control Protocol',
    )
    await expect(desktop.page.getByTestId('conversation')).toContainText(
      'Waiting for permission',
    )
    await expect(desktop.page.getByTestId('session-panel-zone')).toHaveCount(0)
    await current.locator('[data-session-id="session-desktop"]').click()
    await expect(desktop.page.getByTestId('conversation')).toContainText(
      'Build a durable desktop workbench',
    )
    await expect(desktop.page.getByTestId('conversation')).not.toContainText(
      'Waiting for permission',
    )
    for (const [tab, pane] of [
      ['Agents', 'agents-pane'],
      ['Tasks', 'tasks-pane'],
      ['Diagnostics', 'diagnostics-pane'],
    ] as const) {
      await activatePanel(desktop.page, tab)
      await expect(desktop.page.getByTestId(pane)).toBeVisible()
    }
  } finally {
    await closeDesktop(desktop)
  }
})

test('sends work and renders the event-driven response and artifact', async () => {
  const desktop = await launchDesktop('fake')
  try {
    const composer = desktop.page.getByLabel('Message Loopal')
    await composer.fill('Create a verified result')
    await desktop.page.getByRole('button', { name: 'Send' }).click()
    await expect(desktop.page.getByTestId('conversation')).toContainText(
      'Create a verified result',
    )
    await expect(desktop.page.getByTestId('conversation')).toContainText(
      'Loopal handled this message inside the selected session runtime',
    )
    await activatePanel(desktop.page, 'Artifacts')
    await expect(desktop.page.getByTestId('artifacts-pane')).toContainText(
      'Execution summary.md',
    )
  } finally {
    await closeDesktop(desktop)
  }
})

test('connects session, diagnostics, artifact, and agent affordances', async () => {
  const desktop = await launchDesktop('fake')
  try {
    await expect(desktop.page.getByTestId('active-session-title')).toHaveText(
      'Build LoopalDesktop foundation',
    )
    await desktop.page.getByRole('button', { name: 'Session details' }).click()
    await expect(desktop.page.getByTestId('session-metadata')).toContainText('session-desktop')
    await expect(desktop.page.getByTestId('session-metadata')).toContainText('gpt-5')

    await desktop.page.getByRole('button', { name: 'Settings' }).click()
    await selectSettingsSection(desktop.page, 'runtime')
    await expect(desktop.page.getByTestId('settings-pane')
      .getByTestId('diagnostics-pane')).toContainText('Desktop Host')
    await desktop.page.getByRole('button', { name: 'Close settings' }).click()
    await activatePanel(desktop.page, 'Agents')
    const child = desktop.page.locator('[data-agent-id="agent-e2e"]')
    await expect(child).toHaveAttribute('data-parent-id', 'agent-root')
    await expect(child).toContainText('child of Loopal')
    await child.click()
    await expect(desktop.page.getByTestId('conversation')).toContainText(
      'Viewing E2E specialist · waiting',
    )
    await expect(desktop.page.getByTestId('conversation')).toContainText(
      'E2E specialist verified the Electron renderer.',
    )

    const search = desktop.page.getByLabel('Search sessions')
    await search.fill('audit reference')
    await desktop.page.getByTestId('history-session-list')
      .locator('[data-session-id="session-audit"]').click()
    await search.fill('')
    await activatePanel(desktop.page, 'Artifacts')
    const artifact = desktop.page.getByRole('button', { name: /Architecture findings\.md/ })
    await artifact.click()
    await expect(artifact).toHaveAttribute('aria-expanded', 'true')
    await expect(desktop.page.getByTestId('artifacts-pane')).toContainText(
      'loopal-artifact://session-audit/findings.md',
    )
  } finally {
    await closeDesktop(desktop)
  }
})

async function activatePanel(page: import('@playwright/test').Page, name: string): Promise<void> {
  const tab = page.getByRole('tab', { name, exact: true })
  await tab.click()
  await expect(tab).toHaveAttribute('aria-selected', 'true')
  await expect(page.locator('[role="tabpanel"]:visible')).toHaveCount(1)
}

test('removes code and terminal tools while conversation and Federation remain usable', async () => {
  const desktop = await launchDesktop('fake')
  try {
    const page = desktop.page
    for (const name of ['Explorer', 'Search', 'Source Control', 'Terminal']) {
      await expect(page.getByRole('button', { name, exact: true })).toHaveCount(0)
    }
    for (const testId of [
      'explorer-sidebar', 'explorer-tree', 'search-sidebar', 'search-results',
      'source_control-sidebar', 'source-changes', 'file-editor', 'diff-editor',
      'worktree-list', 'terminal-panel',
    ]) await expect(page.getByTestId(testId)).toHaveCount(0)
    await expect(page.getByLabel('Search workspace')).toHaveCount(0)
    await expect(page.getByLabel('New worktree name')).toHaveCount(0)

    await expect(page.getByLabel('Message Loopal')).toBeVisible()
    const federation = page.getByRole('button', { name: 'Federation', exact: true })
    await federation.click()
    await expect(page.getByTestId('primary-workspace')).toHaveAttribute(
      'data-workspace', 'federation',
    )
    await page.getByRole('button', { name: 'Conversation', exact: true }).click()
    await expect(page.getByLabel('Message Loopal')).toBeVisible()
    await expect(page.locator('.terminal-panel, .terminal-surface, .xterm')).toHaveCount(0)
  } finally {
    await closeDesktop(desktop)
  }
})
