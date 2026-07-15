import { expect, test, type Locator, type Page } from '@playwright/test'
import { closeDesktop, launchDesktop } from '../../support/electron/electron-fixture'
import { activeDetail, ready, runtimeTarget, send } from '../../support/runtime/llm-e2e-helpers'

test('projects completed, failed, killed, and restart-cleared background processes', async () => {
  const desktop = await launchDesktop('real', 'provider-background-lifecycle')
  try {
    const page = desktop.page
    await ready(page)
    await setBypass(page)
    await send(page, 'Run deterministic background lifecycles')
    await expect(page.getByTestId('conversation')).toContainText(
      'Background lifecycle processes are running.', { timeout: 20_000 },
    )
    await ready(page)

    const tab = page.getByRole('tab', { name: 'Background', exact: true })
    await expect(tab).toBeVisible()
    await tab.click()
    const pane = page.getByTestId('background-tasks-pane')
    const success = task(pane, 'Successful background fixture')
    const failure = task(pane, 'Failed background fixture')
    const guard = task(pane, 'Killable background fixture')
    await expect(guard).toContainText('running')
    await expect(guard).toContainText('BG_GUARD_START', { timeout: 15_000 })
    await expect(success).toContainText('completed', { timeout: 20_000 })
    await expect(success).toContainText('BG_SUCCESS_DONE')
    await expect(success).toContainText('Exit code 0')
    await expect(failure).toContainText('failed', { timeout: 20_000 })
    await expect(failure).toContainText('BG_FAILURE_ERR')
    await expect(failure).toContainText('Exit code 7')

    await guard.getByRole('button', {
      name: 'Kill background task Killable background fixture',
    }).click()
    await expect(tab).toHaveCount(0, { timeout: 20_000 })
    await expect.poll(async () => backgroundState(page)).toEqual([
      expect.objectContaining({ description: 'Failed background fixture', status: 'failed', exitCode: 7 }),
      expect.objectContaining({ description: 'Killable background fixture', status: 'killed' }),
      expect.objectContaining({ description: 'Successful background fixture', status: 'completed', exitCode: 0 }),
    ])

    const initial = await runtimeTarget(page)
    await page.evaluate(
      (sessionId) => window.loopalDesktop.restartSession(sessionId), initial.sessionId,
    )
    await expect.poll(async () => (await runtimeTarget(page)).generation, {
      timeout: 30_000,
    }).toBe(initial.generation + 1)
    await ready(page)
    await expect(page.getByRole('tab', { name: 'Background', exact: true })).toHaveCount(0)
    expect((await activeDetail(page)).view?.backgroundTasks).toEqual([])

    const requests = await desktop.llm!.requests()
    expect(requests).toHaveLength(2)
    expect(requests[1]!.toolResultIds).toEqual(expect.arrayContaining([
      'background-success', 'background-failure', 'background-guard',
    ]))
    await expect.poll(() => desktop.llm!.state()).toMatchObject({
      name: 'provider-background-lifecycle', served: 2, remaining: 0,
      unmatchedRequests: 0, inFlight: 0, verified: true,
    })
  } finally {
    await closeDesktop(desktop)
  }
})

function task(pane: Locator, description: string): Locator {
  return pane.locator('.background-task').filter({ hasText: description })
}

async function backgroundState(page: Page) {
  return (await activeDetail(page)).view?.backgroundTasks.map((item) => ({
    description: item.description, status: item.status,
    exitCode: item.exitCode, output: item.output,
  })).sort((left, right) => left.description.localeCompare(right.description))
}

async function setBypass(page: Page): Promise<void> {
  const target = await runtimeTarget(page)
  await page.evaluate(async (value) => window.loopalDesktop.controlAgent({
    target: value, command: { type: 'permission', mode: 'bypass' },
  }), target)
}
