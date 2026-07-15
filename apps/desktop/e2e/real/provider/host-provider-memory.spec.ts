import { expect, test, type Locator, type Page } from '@playwright/test'
import { readFile, readdir } from 'node:fs/promises'
import { join } from 'node:path'
import { closeDesktop, launchDesktop } from '../../support/electron/electron-fixture'
import { queueSessionDirectories } from '../../support/fixtures/session-directory-fixture'
import { createFromDirectory } from '../../support/fixtures/session-directory-ui'
import { activeDetail, ready, runtimeTarget, send } from '../../support/runtime/llm-e2e-helpers'
import { selectSettingsSection } from '../../support/settings/settings-helpers'

test('runs observation, recall, and importance through isolated project memory', async () => {
  const desktop = await launchDesktop(
    'real', 'provider-memory', {
      LOOPAL_DESKTOP_E2E_USER_SETTINGS: JSON.stringify({ memory: { enabled: false } }),
    }, 'anthropic', 'memory',
  )
  try {
    const page = desktop.page
    const conversation = page.getByTestId('conversation')
    await ready(page)
    const original = await runtimeTarget(page)

    await page.getByRole('button', { name: 'Settings' }).click()
    await selectSettingsSection(page, 'loopal')
    const pane = page.getByTestId('loopal-default-settings')
    const enabled = pane.getByLabel('Enable project memory')
    await expect(enabled).not.toBeChecked()
    await enabled.check()
    await pane.getByRole('button', { name: 'Save Loopal defaults' }).click()
    await expect(pane.getByRole('status')).toContainText('new or restarted Sessions')
    await page.getByRole('button', { name: 'Close settings' }).click()

    await send(page, 'ACTIVE_SESSION_MEMORY_DISABLED')
    await expect(conversation).toContainText(
      'The already-running Session kept memory disabled.', { timeout: 20_000 },
    )
    const disabled = invocation(conversation, 'ACTIVE_SESSION_MEMORY_DISABLED')
    await expect(disabled.getByLabel('Failed')).toBeVisible()
    await expect(disabled).toContainText('Memory is not enabled')
    await page.waitForTimeout(700)
    expect(await desktop.llm!.state()).toMatchObject({ served: 2 })
    await expect(readFile(join(
      desktop.project, '.loopal/memory/disabled-observation.md',
    ), 'utf8')).rejects.toThrow()

    await queueSessionDirectories(desktop, [desktop.project])
    await createFromDirectory(page, 'directory', true)
    await expect(page.locator('[data-session-id]')).toHaveCount(2, { timeout: 30_000 })
    await ready(page)
    const created = await runtimeTarget(page)
    expect(created.sessionId).not.toBe(original.sessionId)

    await send(page, 'NEW_SESSION_MEMORY_OBSERVATION')
    await expect(conversation).toContainText(
      'The new Session queued the memory observation.', { timeout: 20_000 },
    )
    await expectSuccessfulOutput(
      invocation(conversation, 'NEW_SESSION_MEMORY_OBSERVATION'), 'Noted.',
    )
    await expect.poll(() => desktop.llm!.state(), { timeout: 40_000 }).toMatchObject({ served: 7 })
    const memoryPath = join(
      desktop.project, '.loopal/memory/memory-observer-roundtrip.md',
    )
    await expect.poll(() => readFile(memoryPath, 'utf8'), { timeout: 20_000 })
      .toContain('OBSERVER PERSISTED CONTRACT')
    const firstMemoryAgents = await waitForMemoryAgent(page, new Set())

    await restart(page, created.generation)
    await send(page, 'RESTARTED_SESSION_MEMORY_OBSERVATION')
    await expect(conversation).toContainText(
      'The restarted Session queued another observation.', { timeout: 20_000 },
    )
    await expectSuccessfulOutput(
      invocation(conversation, 'RESTARTED_SESSION_MEMORY_OBSERVATION'), 'Noted.',
    )
    await expect.poll(() => desktop.llm!.state(), { timeout: 40_000 })
      .toMatchObject({ served: 10 })
    await waitForMemoryAgent(page, new Set(firstMemoryAgents))

    await send(page, 'RECALL_OBSERVER_MEMORY_BEFORE_IMPORTANCE')
    await expect(conversation).toContainText(
      'The model received the recalled observer memory.', { timeout: 20_000 },
    )
    const before = await toolOutput(invocation(conversation, 'memory_recall').last())
    expect(before).toContain('OBSERVER PERSISTED CONTRACT')
    expect(before.indexOf('memory-observer-roundtrip'))
      .toBeLessThan(before.indexOf('stable-desktop-contract'))

    await send(page, 'TAG_BASELINE_MEMORY_IMPORTANCE')
    await expect(conversation).toContainText(
      'The importance result reached the model.', { timeout: 20_000 },
    )
    await expectSuccessfulOutput(
      invocation(conversation, 'memory_set_importance').last(),
      'Tagged `stable-desktop-contract` with importance=10',
    )
    await expect.poll(async () => (await memoryEvents(desktop.project)).some((event) => (
      event.type === 'importance_tag' && event.node === 'stable-desktop-contract'
        && event.importance === 10
    ))).toBe(true)

    const afterImportance = await runtimeTarget(page)
    await restart(page, afterImportance.generation)
    await send(page, 'RECALL_MEMORY_AFTER_RESTART')
    await expect(conversation).toContainText(
      'Restarted recall preserved memory and importance ordering.', { timeout: 20_000 },
    )
    const after = await toolOutput(invocation(conversation, 'memory_recall').last())
    expect(after).toContain('OBSERVER PERSISTED CONTRACT')
    expect(after.indexOf('stable-desktop-contract'))
      .toBeLessThan(after.indexOf('memory-observer-roundtrip'))

    const requests = await desktop.llm!.requests()
    expect(requests).toHaveLength(16)
    expect(requests[0]!.toolNames).toEqual(expect.arrayContaining([
      'Memory', 'memory_recall', 'memory_set_importance',
    ]))
    expect(requests[1]!.toolResultErrorIds).toContain('memory-disabled')
    expect(requests[3]!.toolResultIds).toContain('memory-new-session')
    expect(requests[8]!.toolResultIds).toContain('memory-restarted-session')
    expect(requests[11]!.toolResultIds).toContain('main-recall-before')
    expect(requests[13]!.toolResultIds).toContain('main-importance')
    expect(requests[15]!.toolResultIds).toContain('main-recall-after')
    await expect.poll(() => desktop.llm!.state()).toMatchObject({
      served: 16, remaining: 0, verified: true,
    })
  } finally {
    await closeDesktop(desktop)
  }
})

function invocation(conversation: Locator, text: string): Locator {
  return conversation.getByTestId('tool-invocation').filter({ hasText: text }).last()
}

async function expectSuccessfulOutput(tool: Locator, output: string): Promise<void> {
  await expect(tool.getByLabel('Completed')).toBeVisible({ timeout: 20_000 })
  if (!(await tool.evaluate((node) => (node as HTMLDetailsElement).open))) {
    await tool.locator(':scope > summary').click()
  }
  await expect(tool.locator('.tool-output')).toContainText(output)
}

async function toolOutput(tool: Locator): Promise<string> {
  await expect(tool.getByLabel('Completed')).toBeVisible({ timeout: 20_000 })
  if (!(await tool.evaluate((node) => (node as HTMLDetailsElement).open))) {
    await tool.locator(':scope > summary').click()
  }
  return await tool.locator('.tool-output').innerText()
}

async function restart(page: Page, generation: number): Promise<void> {
  await page.getByRole('button', { name: 'Restart session', exact: true }).click()
  await expect.poll(() => runtimeTarget(page).then((target) => target.generation), {
    timeout: 30_000,
  }).toBe(generation + 1)
  await ready(page)
}

async function waitForMemoryAgent(page: Page, previous: Set<string>): Promise<string[]> {
  const completed = async () => (await activeDetail(page)).agents.filter((agent) => (
    agent.name.startsWith('memory-') && agent.status === 'completed'
  )).map((agent) => agent.name)
  await expect.poll(async () => (await completed()).some((name) => !previous.has(name)), {
    timeout: 30_000,
  }).toBe(true)
  return completed()
}

async function memoryEvents(project: string): Promise<Record<string, unknown>[]> {
  const directory = join(project, '.loopal/memory-events')
  const files = (await readdir(directory)).filter((name) => name.endsWith('.jsonl'))
  const contents = await Promise.all(files.map((name) => readFile(join(directory, name), 'utf8')))
  return contents.flatMap((content) => content.trim().split('\n'))
    .filter(Boolean).map((line) => JSON.parse(line) as Record<string, unknown>)
}
