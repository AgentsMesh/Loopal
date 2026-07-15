import { expect, test, type Page } from '@playwright/test'
import { readFile } from 'node:fs/promises'
import { join } from 'node:path'
import { closeDesktop, launchDesktop, waitForHostStatus } from '../../support/electron/electron-fixture'
import { activeDetail, ready } from '../../support/runtime/llm-e2e-helpers'

test('projects production Goal, Task, Background, Cron, and Artifact lifecycles', async () => {
  const desktop = await launchDesktop('real', 'runtime-resources')
  try {
    const page = desktop.page
    await waitForHostStatus(page, 'ready')
    await setPermissionBypass(page)
    await page.getByLabel('Message Loopal').fill('Create deterministic runtime resources')
    await page.getByRole('button', { name: 'Send' }).click()
    await expect(page.getByTestId('conversation')).toContainText(
      'All deterministic runtime resources are ready.', { timeout: 30_000 },
    )

    await activate(page, 'Tasks')
    const tasks = page.getByTestId('tasks-pane')
    await expect(tasks).toContainText('Verify Desktop runtime resource panels')
    await expect(tasks).toContainText('Inspect runtime resource panels')
    await expect(tasks).toContainText('Complete')

    await activate(page, 'Background')
    const background = page.getByTestId('background-tasks-pane')
    await expect(background).toContainText('Desktop fixture background process')
    await background.getByRole('button', { name: /Kill background task/ }).click()
    await expect(page.getByRole('tab', { name: 'Background', exact: true })).toHaveCount(0)

    await activate(page, 'Scheduled')
    const scheduled = page.getByTestId('scheduled-work-pane')
    await expect(scheduled).toContainText('Run the deterministic Desktop fixture check')
    await scheduled.getByRole('button', { name: /Delete scheduled work/ }).click()
    await expect(page.getByRole('tab', { name: 'Scheduled', exact: true })).toHaveCount(0)

    await activate(page, 'Artifacts')
    await expect(page.getByTestId('artifacts-pane')).toContainText('runtime-fixture.txt')
    await expect.poll(() => readFile(join(desktop.project, 'runtime-fixture.txt'), 'utf8'))
      .toBe('runtime fixture artifact\n')

    const requests = await desktop.llm!.requests()
    expect(requests).toHaveLength(7)
    expect(requests[0]!.toolNames).toEqual(expect.arrayContaining([
      'create_goal', 'update_goal', 'TaskCreate', 'Bash', 'CronCreate', 'Write',
    ]))
    expect(requests.every((request) => request.matched)).toBe(true)
    await expect.poll(() => desktop.llm!.state()).toMatchObject({
      name: 'runtime-resources', remaining: 0, verified: true,
    })
  } finally {
    await closeDesktop(desktop)
  }
})

test('auto-continues an active Goal without adding a human message', async () => {
  const desktop = await launchDesktop('real', 'provider-goal-loop')
  try {
    const page = desktop.page
    const conversation = page.getByTestId('conversation')
    await waitForHostStatus(page, 'ready')
    await setPermissionBypass(page)
    await page.getByLabel('Message Loopal').fill('Start a deterministic persistent goal')
    await page.getByRole('button', { name: 'Send' }).click()
    await expect(conversation).toContainText(
      'Goal continuation completed without another human turn.', { timeout: 30_000 },
    )
    await expect(conversation.locator('[data-message-role="user"]')).toHaveCount(1)
    const requests = await desktop.llm!.requests()
    expect(requests).toHaveLength(4)
    expect(requests[2]!.lastUserText).toContain('Continue working toward the active thread goal')
    expect(requests[2]!.lastUserText).toContain('Exercise the autonomous Desktop goal loop')
    const detail = await activeDetail(page)
    expect(detail.view?.goal?.status).toBe('complete')
    expect(detail.agents.find((agent) => agent.id === 'main')?.telemetry?.turnCount).toBe(2)
    await expect(page.getByRole('tab', { name: 'Tasks', exact: true })).toHaveCount(0)
    await ready(page)
    await page.waitForTimeout(300)
    await expect.poll(() => desktop.llm!.state()).toMatchObject({
      served: 4, requestCount: 4, remaining: 0, unmatchedRequests: 0, verified: true,
    })
  } finally {
    await closeDesktop(desktop)
  }
})

async function setPermissionBypass(page: Page): Promise<void> {
  const target = await page.evaluate(async () => {
    const bootstrap = await window.loopalDesktop.bootstrap()
    const sessionId = bootstrap.activeSessionId!
    const runtime = bootstrap.runtimes.find((item) => item.sessionId === sessionId)!
    return {
      sessionId, runtimeId: runtime.id, generation: runtime.generation,
      agentId: runtime.rootAgent,
    }
  })
  await page.evaluate(async (value) => {
    await window.loopalDesktop.controlAgent({
      target: value, command: { type: 'permission', mode: 'bypass' },
    })
  }, target)
  await expect.poll(async () => page.evaluate(async (value) => {
    const detail = await window.loopalDesktop.openSession(value.sessionId)
    return detail.agents.find((agent) => agent.id === value.agentId)?.permissionMode
  }, target)).toBe('bypass')
}

async function activate(page: Page, name: string): Promise<void> {
  const tab = page.getByRole('tab', { name, exact: true })
  await tab.click()
  await expect(tab).toHaveAttribute('aria-selected', 'true')
}
