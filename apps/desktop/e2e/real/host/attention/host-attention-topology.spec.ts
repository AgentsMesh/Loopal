import { expect, test, type Page } from '@playwright/test'
import { readFile } from 'node:fs/promises'
import { join } from 'node:path'
import {
  closeDesktop, launchDesktop, waitForHostStatus,
} from '../../../support/electron/electron-fixture'
import { selectSettingsSection } from '../../../support/settings/settings-helpers'

test('resolves real questions, approvals, and retained child topology', async () => {
  const desktop = await launchDesktop('real', 'attention-topology')
  try {
    const page = desktop.page
    await waitForHostStatus(page, 'ready')
    await expect(page.getByTestId('runtime-status')).toContainText(
      'Ready for input', { timeout: 30_000 },
    )
    await expect(page.getByLabel('Message Loopal')).toBeEnabled({ timeout: 30_000 })
    await expect(page.getByRole('tab', { name: 'Agents', exact: true })).toHaveCount(0)
    const target = await configureInteractiveRuntime(page)

    await send(page, 'Ask me which verification path to use')
    const questions = page.getByTestId('questions-pane')
    await expect(questions).toContainText('Verification: Choose a verification path')
    await expect(questions).toContainText('Auto-answering')
    await questions.getByRole('button', { name: /Thorough/ }).click()
    await questions.getByRole('button', { name: 'Submit answers' }).click()
    await expect(questions).toHaveCount(0)
    await expect(page.getByTestId('conversation')).toContainText(
      'Question answer reached the real runtime.', { timeout: 20_000 },
    )
    await setDecisionMode(page, target, 'manual')

    await send(page, 'Try a write that I will deny')
    const approvals = page.getByTestId('permissions-pane')
    await expect(approvals).toContainText('Allow Write')
    await approvals.getByRole('button', { name: 'Deny' }).click()
    await expect(page.getByTestId('conversation')).toContainText(
      'Denied write was handled and the runtime continued.', { timeout: 20_000 },
    )
    await expect(readFile(join(desktop.project, 'denied.txt'), 'utf8')).rejects.toThrow()

    await send(page, 'Perform the approved write')
    await expect(approvals).toContainText('Allow Write')
    await approvals.getByRole('button', { name: 'Allow', exact: true }).click()
    await expect(page.getByTestId('conversation')).toContainText(
      'Approved write completed through the real runtime.', { timeout: 20_000 },
    )
    await expect.poll(() => readFile(join(desktop.project, 'approved.txt'), 'utf8'))
      .toBe('real approval path\n')
    const artifactsTab = page.getByRole('tab', { name: 'Artifacts', exact: true })
    await artifactsTab.click()
    await expect(page.getByTestId('artifacts-pane')).toContainText('approved.txt')

    await send(page, 'Spawn the retained desktop child')
    await expect(approvals).toContainText('Allow Agent')
    await approvals.getByRole('button', { name: 'Allow', exact: true }).click()
    await expect(questions).toContainText('Choose a verification path', { timeout: 30_000 })
    await expect(questions).toContainText('Agent question · desktop-child')
    await questions.getByRole('button', { name: /Fast/ }).click()
    await questions.getByRole('button', { name: 'Submit answers' }).click()
    await expect(page.getByTestId('conversation')).toContainText(
      'Root received the retained child result.', { timeout: 40_000 },
    )

    await expect(artifactsTab).toHaveAttribute('aria-selected', 'true')
    const agentsTab = page.getByRole('tab', { name: 'Agents', exact: true })
    await agentsTab.click()
    const agents = page.getByTestId('agents-pane')
    await agents.getByRole('button', { name: 'All', exact: true }).click()
    const child = agents.locator('[data-agent-id="desktop-child"]')
    await expect(child).toContainText('completed', { timeout: 30_000 })
    await child.click()
    const conversation = page.getByTestId('conversation')
    await expect(conversation).toContainText('Viewing desktop-child · completed')
    await expect(conversation).toContainText('Finish after answering the verification question.')
    await expect(conversation).toContainText('Q1 (Choose a verification path): Fast')
    await expect(conversation).toContainText('CHILD RESPONSE RETAINED')
    await expect(page.getByLabel('Message desktop-child')).toBeDisabled()
    await page.getByRole('button', { name: 'Settings' }).click()
    await selectSettingsSection(page, 'agent')
    await expect(page.getByRole('group', { name: 'Agent controls' })
      .getByRole('button', { name: 'Clear' })).toBeDisabled()

    const detail = await page.evaluate(
      (id) => window.loopalDesktop.openSession(id), target.sessionId,
    )
    expect(detail.agents.find((agent) => agent.id === 'desktop-child')?.conversation?.length)
      .toBeGreaterThan(0)
    await expect.poll(() => desktop.llm!.requests()).toHaveLength(11)
    const requests = await desktop.llm!.requests()
    expect(requests.every((request) => request.matched && request.apiKeyPresent)).toBe(true)
    expect(requests.some((request) => (
      request.lastUserText.includes('Finish after answering the verification question.')
    ))).toBe(true)
    expect(requests.some((request) => request.toolResultIds.includes('child-ask-path'))).toBe(true)
    await expect.poll(() => desktop.llm!.state()).toMatchObject({
      name: 'attention-topology', remaining: 0, verified: true,
    })
  } finally {
    await closeDesktop(desktop)
  }
})

interface RuntimeTarget {
  readonly sessionId: string
  readonly runtimeId: string
  readonly generation: number
  readonly agentId: string
}

async function configureInteractiveRuntime(page: Page): Promise<RuntimeTarget> {
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
      target: value, command: { type: 'decision', mode: 'classifier' },
    })
    await window.loopalDesktop.controlAgent({
      target: value, command: { type: 'permission', mode: 'ask_any_write' },
    })
  }, target)
  await expect.poll(async () => page.evaluate(async (value) => {
    const detail = await window.loopalDesktop.openSession(value.sessionId)
    const root = detail.agents.find((agent) => agent.id === value.agentId)
    return `${root?.decisionMode}/${root?.permissionMode}`
  }, target)).toBe('classifier/ask_any_write')
  return target
}

async function setDecisionMode(
  page: Page, target: RuntimeTarget, mode: 'manual' | 'classifier',
): Promise<void> {
  await page.evaluate(async ({ value, nextMode }) => {
    await window.loopalDesktop.controlAgent({
      target: value, command: { type: 'decision', mode: nextMode },
    })
  }, { value: target, nextMode: mode })
  await expect.poll(async () => page.evaluate(async (value) => {
    const detail = await window.loopalDesktop.openSession(value.sessionId)
    return detail.agents.find((agent) => agent.id === value.agentId)?.decisionMode
  }, target)).toBe(mode)
}

async function send(page: Page, text: string): Promise<void> {
  await page.getByLabel('Message Loopal').fill(text)
  await page.getByRole('button', { name: 'Send' }).click()
  await expect(page.getByTestId('conversation')).toContainText(text)
}
