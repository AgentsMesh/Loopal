import { expect, test } from '@playwright/test'
import { closeDesktop, launchDesktop } from '../../support/electron/electron-fixture'
import { ready, send } from '../../support/runtime/llm-e2e-helpers'
import { selectSettingsSection } from '../../support/settings/settings-helpers'

test('interrupts a real background child through the selected Agent UI', async () => {
  const desktop = await launchDesktop('real', 'subagent-ui-interrupt')
  try {
    const page = desktop.page
    const conversation = page.getByTestId('conversation')
    await ready(page)

    await page.getByRole('button', { name: 'Settings' }).click()
    await selectSettingsSection(page, 'agent')
    const rootControls = page.getByRole('group', { name: 'Agent controls' })
    await rootControls.getByLabel('Permission mode').selectOption('bypass')
    await expect(rootControls.getByLabel('Permission mode')).toHaveValue('bypass')
    await page.getByRole('button', { name: 'Close settings' }).click()

    await send(page, 'Spawn the UI interrupt child')
    await expect(conversation).toContainText('UI child is running in background.', {
      timeout: 30_000,
    })
    const agentsTab = page.getByRole('tab', { name: 'Agents', exact: true })
    await agentsTab.click()
    const agents = page.getByTestId('agents-pane')
    const child = agents.locator('[data-agent-id="ui-interrupt-child"]')
    await expect(child).toContainText('running', { timeout: 30_000 })
    await child.click()
    await expect(child).toHaveAttribute('aria-selected', 'true')
    await expect(conversation).toContainText('Viewing ui-interrupt-child · running')
    await expect(conversation).toContainText('UI CHILD STREAM ACTIVE', { timeout: 20_000 })

    await page.getByRole('button', { name: 'Settings' }).click()
    await selectSettingsSection(page, 'agent')
    await expect(page.getByLabel('Settings agent')).toHaveValue('ui-interrupt-child')
    const childControls = page.getByRole('group', { name: 'Agent controls' })
    await expect(childControls).toContainText('ui-interrupt-child')
    const interrupt = childControls.getByRole('button', { name: 'Interrupt' })
    await expect(interrupt).toBeEnabled()
    await interrupt.click()
    await expect.poll(() => desktop.llm!.state()).toMatchObject({
      clientDisconnects: 1, inFlight: 0,
    })
    await page.getByRole('button', { name: 'Close settings' }).click()

    await expect(conversation).toContainText('Viewing ui-interrupt-child · failed', {
      timeout: 30_000,
    })
    await expect(conversation).toContainText('UI CHILD STREAM ACTIVE')
    await expect(conversation).not.toContainText('UI CHILD LATE OUTPUT')
    await expect(child).toContainText('failed')

    await agents.locator('[data-agent-id="main"]').click()
    await expect(page.getByLabel('Message Loopal')).toBeEnabled()
    await expect(conversation).toContainText('Root observed the UI child interrupt.', {
      timeout: 30_000,
    })
    await send(page, 'Continue after the UI child interrupt')
    await expect(conversation).toContainText('Root remained usable after the UI interrupt.', {
      timeout: 20_000,
    })
    await assertJournal(desktop)
  } finally {
    await closeDesktop(desktop)
  }
})

async function assertJournal(desktop: Awaited<ReturnType<typeof launchDesktop>>): Promise<void> {
  await expect.poll(() => desktop.llm!.state()).toMatchObject({
    name: 'subagent-ui-interrupt', served: 5, requestCount: 5,
    remaining: 0, unmatchedRequests: 0, inFlight: 0,
    clientDisconnects: 1, verified: true,
  })
  const requests = await desktop.llm!.requests()
  expect(requests.map(({ sequence, matched }) => ({ sequence, matched }))).toEqual([
    { sequence: 1, matched: true }, { sequence: 2, matched: true },
    { sequence: 3, matched: true }, { sequence: 4, matched: true },
    { sequence: 5, matched: true },
  ])
  expect(requests.filter((request) => (
    request.lastUserText === 'Spawn the UI interrupt child'
  ))).toHaveLength(1)
  expect(matching(requests, 'Hold the UI interrupt child stream')).toHaveLength(1)
  expect(requests.filter((request) => (
    request.lastUserText === ''
      && request.toolResultIds.includes('spawn-ui-interrupt')
  ))).toHaveLength(1)
  expect(matching(requests, 'UI CHILD STREAM ACTIVE')).toHaveLength(1)
  expect(requests.filter((request) => (
    request.lastUserText === 'Continue after the UI child interrupt'
  ))).toHaveLength(1)
  expect(JSON.stringify(requests)).not.toContain('UI CHILD LATE OUTPUT')
}

function matching(
  requests: Awaited<ReturnType<NonNullable<Awaited<ReturnType<typeof launchDesktop>>['llm']>['requests']>>,
  marker: string,
) {
  return requests.filter((request) => request.lastUserText.includes(marker))
}
