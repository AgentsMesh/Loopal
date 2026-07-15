import { expect, test } from '@playwright/test'
import {
  closeDesktop, launchDesktop, waitForHostStatus,
} from '../../support/electron/electron-fixture'

test('renders rich state from the real Loopal Host', async () => {
  const desktop = await launchDesktop('real', 'production-rich')
  try {
    await waitForHostStatus(desktop.page, 'ready')
    await expect(desktop.page.getByRole('tab', { name: 'Tasks', exact: true })).toHaveCount(0)
    const composer = desktop.page.getByLabel('Message Loopal')
    await composer.fill('Inspect the project and report with rich formatting')
    await desktop.page.getByRole('button', { name: 'Send' }).click()

    const conversation = desktop.page.getByTestId('conversation')
    await expect(conversation.getByText('Inspecting the real Host workspace.')).toBeVisible({
      timeout: 15_000,
    })
    await expect(desktop.page.getByTestId('runtime-status')).toContainText('Thinking')

    const readTool = conversation.getByTestId('tool-invocation').filter({ hasText: /^Read/ })
    await expect(readTool).toBeVisible({ timeout: 15_000 })
    await expect(readTool.getByLabel('Completed')).toBeVisible({ timeout: 15_000 })
    await readTool.locator(':scope > summary').click()
    await expect(readTool).toContainText('Loopal Desktop E2E')

    await expect(conversation.getByText('Preparing a real tool call.')).toBeVisible()
    const assistantStream = conversation.locator('[data-message-role="assistant"].streaming')
    await expect(assistantStream.getByLabel('Streaming')).toBeVisible()
    await expect(desktop.page.getByTestId('runtime-status')).toContainText('Streaming')

    await expect(conversation.getByRole('heading', { name: 'Real Host verified' })).toBeVisible({
      timeout: 20_000,
    })
    await expect(conversation.getByText('Markdown rendered')).toBeVisible()
    await expect(conversation.getByText('Production stream quote')).toBeVisible()
    await expect(conversation.getByText('fn contract() -> bool { true }')).toBeVisible()
    await expect(conversation.getByRole('link', { name: 'Loopal reference' }))
      .toHaveAttribute('href', 'https://example.com/loopal')
    await expect(conversation.getByText('final streamed chunk')).toBeVisible()
    await expect(desktop.page.getByTestId('runtime-status')).toContainText('Ready for input')
    const detail = await desktop.page.evaluate(async () => {
      const bootstrap = await window.loopalDesktop.bootstrap()
      return window.loopalDesktop.openSession(bootstrap.activeSessionId!)
    })
    const root = detail.agents.find((agent) => agent.id === 'main')!
    expect(root.view?.thinkingActive).toBe(false)
    expect(root.telemetry?.thinkingTokens).toBeGreaterThan(0)
    expect(root.conversation?.some((entry) => (
      entry.role === 'thinking' && entry.text.includes('Inspecting the real Host workspace.')
    ))).toBe(true)

    const taskTab = desktop.page.getByRole('tab', { name: 'Tasks', exact: true })
    await taskTab.click()
    await expect(taskTab).toHaveAttribute('aria-selected', 'true')
    const tasks = desktop.page.getByTestId('tasks-pane')
    await expect(tasks).toContainText('Verify the real Desktop Host')
    await expect(tasks).toContainText('Plan · 0/1')

    const requests = await desktop.llm!.requests()
    expect(requests).toHaveLength(3)
    expect(requests[0]).toMatchObject({
      protocol: 'anthropic', model: 'claude-opus-4-8', stream: true,
      hasSystem: true, apiKeyPresent: true, protocolVersionPresent: true, matched: true,
    })
    expect(requests[0]!.toolNames).toEqual(expect.arrayContaining([
      'Read', 'TaskCreate', 'create_goal',
    ]))
    expect(requests[1]!.toolResultIds).toContain('read-project')
    expect(requests[2]!.toolResultIds).toContain('create-task')
    await expect.poll(() => desktop.llm!.state()).toMatchObject({
      name: 'production-rich', served: 3, remaining: 0, verified: true,
    })
  } finally {
    await closeDesktop(desktop)
  }
})
