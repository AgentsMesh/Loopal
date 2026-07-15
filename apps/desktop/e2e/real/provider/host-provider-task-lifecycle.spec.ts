import { expect, test, type Locator, type Page } from '@playwright/test'
import { readFile } from 'node:fs/promises'
import { join } from 'node:path'
import { closeDesktop, launchDesktop } from '../../support/electron/electron-fixture'
import { activeDetail, ready, runtimeTarget, send } from '../../support/runtime/llm-e2e-helpers'

test('persists model-driven Task dependencies and status transitions across restart', async () => {
  const desktop = await launchDesktop('real', 'provider-task-lifecycle')
  try {
    const page = desktop.page
    const conversation = page.getByTestId('conversation')
    await ready(page)
    await send(page, 'Build a persisted task dependency lifecycle')
    await expect(conversation).toContainText(
      'Task lifecycle is ready for a Session restart.', { timeout: 30_000 },
    )
    await ready(page)
    await assertActiveTasks(page)
    await expect(conversation.getByTestId('tool-invocation').filter({
      hasText: 'Foundation task',
    }).filter({ hasText: '"status": "completed"' })).toHaveCount(1)

    const initial = await runtimeTarget(page)
    const taskFile = join(
      desktop.home, '.loopal', 'sessions', initial.sessionId, 'tasks', 'tasks.json',
    )
    await expect.poll(() => persistedTasks(taskFile)).toEqual([
      { id: '1', status: 'completed' }, { id: '2', status: 'in_progress' },
    ])
    await page.evaluate(
      (sessionId) => window.loopalDesktop.restartSession(sessionId), initial.sessionId,
    )
    await expect.poll(async () => (await runtimeTarget(page)).generation, {
      timeout: 30_000,
    }).toBe(initial.generation + 1)
    await ready(page)
    expect(await persistedTasks(taskFile)).toEqual([
      { id: '1', status: 'completed' }, { id: '2', status: 'in_progress' },
    ])
    await expect.poll(() => activeTasks(page), { timeout: 30_000 }).toEqual([
      { id: '2', status: 'in_progress', blockedBy: ['1'] },
    ])
    await assertActiveTasks(page)

    await send(page, 'Complete the persisted dependent task')
    await expect(conversation).toContainText(
      'Persisted task lifecycle completed after restart.', { timeout: 30_000 },
    )
    await ready(page)
    await expect(page.getByRole('tab', { name: 'Tasks', exact: true })).toHaveCount(0)
    const detail = await activeDetail(page)
    expect(detail.view?.tasks).toEqual([])
    const finalList = conversation.getByTestId('tool-invocation').last()
    await expect(finalList.locator('summary strong')).toContainText('TaskList')
    const finalOutput = finalList.locator('.tool-output')
    await expect(finalOutput).toContainText('"status": "completed"')
    const output = JSON.parse((await finalOutput.textContent()) ?? '') as Array<{
      id: string
      status: string
    }>
    expect(output.map(({ id, status }) => ({ id, status }))).toEqual([
      { id: '1', status: 'completed' }, { id: '2', status: 'completed' },
    ])

    const requests = await desktop.llm!.requests()
    expect(requests).toHaveLength(14)
    expect(requests[5]!.toolResultIds).toContain('task-get-dependent')
    expect(requests[8]!.toolResultIds).toContain('task-dependent-start')
    expect(requests[10]!.toolResultIds).toContain('task-list-after-restart')
    expect(requests[11]!.toolResultIds).toContain('task-get-after-restart')
    expect(requests[13]!.toolResultIds).toContain('task-list-final')
    await expect.poll(() => desktop.llm!.state()).toMatchObject({
      name: 'provider-task-lifecycle', served: 14, remaining: 0,
      unmatchedRequests: 0, inFlight: 0, verified: true,
    })
  } finally {
    await closeDesktop(desktop)
  }
})

async function assertActiveTasks(page: Page): Promise<void> {
  const tab = page.getByRole('tab', { name: 'Tasks', exact: true })
  const pane = page.getByTestId('tasks-pane')
  await expect(tab).toBeVisible()
  await expect(pane).toBeHidden()
  await tab.click()
  await expect(tab).toHaveAttribute('aria-selected', 'true')
  await expect(pane).toBeVisible()
  await expect(pane.getByRole('button')).toHaveCount(0)
  await expect(pane.getByRole('textbox')).toHaveCount(0)
  const foundation = taskRow(page, pane, 'Foundation task')
  const dependent = taskRow(page, pane, 'Dependent task')
  await expect(foundation).toHaveCount(0)
  await expect(dependent).toHaveClass(/task-in_progress/u)
  await dependent.locator(':scope > summary').click()
  await expect(dependent).toContainText('Blocked by 1')
  await expect(dependent).toContainText('Running dependent lifecycle')
  await tab.click()
  await expect(tab).toHaveAttribute('aria-selected', 'true')
  await expect(pane).toBeHidden()
}

function taskRow(page: Page, pane: Locator, subject: string): Locator {
  return pane.locator('.task-row').filter({ has: page.getByText(subject, { exact: true }) })
}

async function activeTasks(page: Page) {
  return (await activeDetail(page)).view?.tasks.map((task) => ({
    id: task.id, status: task.status, blockedBy: task.blockedBy,
  }))
}

async function persistedTasks(path: string) {
  const tasks = JSON.parse(await readFile(path, 'utf8')) as Array<{
    id: string
    status: string
  }>
  return tasks.map(({ id, status }) => ({ id, status }))
}
