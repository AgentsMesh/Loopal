import { expect, test, type Page } from '@playwright/test'
import { closeDesktop, launchDesktop } from '../../support/electron/electron-fixture'
import { activeDetail, ready, runtimeTarget, send } from '../../support/runtime/llm-e2e-helpers'
import { selectSettingsSection } from '../../support/settings/settings-helpers'

test('queues a durable cron while suspended and wakes the idle model after unsuspend', async () => {
  test.setTimeout(180_000)
  const desktop = await launchDesktop('real', 'provider-cron-idle-lifecycle')
  try {
    const page = desktop.page
    const conversation = page.getByTestId('conversation')
    await ready(page)
    await setBypass(page)
    await avoidMinuteBoundary(page)
    await send(page, 'Schedule a durable idle wake')
    const idle = conversation.getByTestId('tool-invocation').filter({ hasText: 'request_idle' })
    await expect(idle.getByLabel('Completed')).toBeVisible({ timeout: 30_000 })
    await expect(conversation).toContainText(
      /Automatic continuation paused: model requested until /u, { timeout: 20_000 },
    )
    await ready(page)

    const dock = page.getByTestId('session-panel-zone')
    await expect(dock.locator('[role="tabpanel"]:visible')).toHaveCount(0)
    await expect(page.getByRole('tab', { name: 'Tasks', exact: true })).toBeVisible()
    const scheduledTab = page.getByRole('tab', { name: 'Scheduled', exact: true })
    await scheduledTab.click()
    const scheduled = page.getByTestId('scheduled-work-pane')
    await expect(scheduled).toBeVisible()
    await expect(scheduled).toContainText('DURABLE CRON WAKE MARKER')
    const before = await cronState(page)
    expect(before).toMatchObject({ recurring: true, durable: true })
    if (!before.nextFireAt) throw new Error('durable cron did not expose its next fire')

    await setSuspended(page, true)
    await expect(conversation).toContainText('Automatic continuation paused: user suspend')
    await waitPast(page, before.nextFireAt)
    await expect.poll(() => desktop.llm!.state()).toMatchObject({
      served: 3, requestCount: 3, remaining: 2,
    })

    await setSuspended(page, false)
    await expect(conversation).toContainText(
      'Durable cron woke the suspended production Session.', { timeout: 30_000 },
    )
    await expect(conversation).toContainText('Automatic continuation resumed.')
    await ready(page)
    const requests = await desktop.llm!.requests()
    expect(requests).toHaveLength(5)
    expect(requests[3]!.lastUserText).toContain('DURABLE CRON WAKE MARKER')
    expect(requests[4]!.toolResultIds).toEqual(expect.arrayContaining([
      'cron-list-after-wake', 'cron-goal-complete',
    ]))

    await scheduledTab.click()
    await expect(dock.locator('[role="tabpanel"]:visible')).toHaveCount(0)
    const initial = await runtimeTarget(page)
    await page.evaluate(
      (sessionId) => window.loopalDesktop.restartSession(sessionId), initial.sessionId,
    )
    await expect.poll(async () => (await runtimeTarget(page)).generation, {
      timeout: 30_000,
    }).toBe(initial.generation + 1)
    await ready(page)
    await expect(page.getByRole('tab', { name: 'Tasks', exact: true })).toHaveCount(0)
    const restoredTab = page.getByRole('tab', { name: 'Scheduled', exact: true })
    await expect(dock.locator('[role="tabpanel"]:visible')).toHaveCount(0)
    await restoredTab.click()
    const restored = page.getByTestId('scheduled-work-pane')
    await expect(restored).toBeVisible()
    await expect(restored).toContainText(
      'DURABLE CRON WAKE MARKER',
    )
    await page.getByRole('button', {
      name: 'Delete scheduled work DURABLE CRON WAKE MARKER',
    }).click()
    await expect(restoredTab).toHaveCount(0)
    await expect.poll(async () => (await activeDetail(page)).view?.crons).toEqual([])
    await expect.poll(() => desktop.llm!.state()).toMatchObject({
      name: 'provider-cron-idle-lifecycle', served: 5, remaining: 0,
      unmatchedRequests: 0, inFlight: 0, verified: true,
    })
  } finally {
    await closeDesktop(desktop)
  }
})

async function cronState(page: Page) {
  const cron = (await activeDetail(page)).view?.crons[0]
  if (!cron) throw new Error('scheduled cron did not reach the Desktop projection')
  return cron
}

async function setBypass(page: Page): Promise<void> {
  const target = await runtimeTarget(page)
  await page.evaluate(async (value) => window.loopalDesktop.controlAgent({
    target: value, command: { type: 'permission', mode: 'bypass' },
  }), target)
}

async function setSuspended(page: Page, suspended: boolean): Promise<void> {
  await page.getByRole('button', { name: 'Settings' }).click()
  await selectSettingsSection(page, 'agent')
  const controls = page.getByRole('group', { name: 'Agent controls' })
  await controls.getByRole('button', { name: suspended ? 'Suspend' : 'Unsuspend' }).click()
  await expect(controls.getByRole('button', {
    name: suspended ? 'Unsuspend' : 'Suspend',
  })).toBeVisible({ timeout: 20_000 })
  await page.getByRole('button', { name: 'Close settings' }).click()
  await expect(page.getByTestId('runtime-status')).toContainText(
    suspended ? 'Suspended' : /Running|Ready for input/u, { timeout: 20_000 },
  )
}

async function avoidMinuteBoundary(page: Page): Promise<void> {
  const seconds = new Date().getUTCSeconds()
  if (seconds >= 43) await page.waitForTimeout((62 - seconds) * 1_000)
}

async function waitPast(page: Page, timestamp: string): Promise<void> {
  const delay = Math.max(0, new Date(timestamp).getTime() - Date.now() + 2_000)
  await page.waitForTimeout(delay)
}
