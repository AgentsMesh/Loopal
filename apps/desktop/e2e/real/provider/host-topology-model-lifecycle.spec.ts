import { expect, test, type Page } from '@playwright/test'
import { closeDesktop, launchDesktop } from '../../support/electron/electron-fixture'
import { ready, runtimeTarget, send } from '../../support/runtime/llm-e2e-helpers'

async function ensureAgentsPanelOpen(page: Page) {
  const agents = page.getByTestId('agents-pane')
  if (!await agents.isVisible()) {
    await page.getByRole('tab', { name: 'Agents', exact: true }).click()
  }
  await expect(agents).toBeVisible()
  return agents
}

test('retains parallel, failed, and nested model-driven Agent topology', async () => {
  const desktop = await launchDesktop('real', 'topology-model-lifecycle')
  try {
    const page = desktop.page
    await ready(page)
    const target = await runtimeTarget(page)
    await page.evaluate(async (value) => window.loopalDesktop.controlAgent({
      target: value, command: { type: 'permission', mode: 'bypass' },
    }), target)
    const conversation = page.getByTestId('conversation')

    await send(page, 'Spawn parallel model children')
    await expect(conversation).toContainText(
      'Parallel child results returned to root.', { timeout: 50_000 },
    )
    await ready(page)
    await send(page, 'Spawn a failing model child')
    await expect(conversation).toContainText(
      'Failed child result returned to root.', { timeout: 40_000 },
    )
    await ready(page)
    await send(page, 'Spawn a nested model child')
    await expect(conversation).toContainText(
      'Nested child result returned to root.', { timeout: 60_000 },
    )
    await ready(page)

    const agents = await ensureAgentsPanelOpen(page)
    await agents.getByRole('button', { name: 'All', exact: true }).click()
    for (const id of [
      'parallel-alpha', 'parallel-beta', 'failed-child', 'parent-child', 'nested-grandchild',
    ]) {
      await expect(agents.locator(`[data-agent-id="${id}"]`)).toBeVisible()
    }
    await expect(agents.locator('[data-agent-id="parallel-alpha"]')).toContainText('completed')
    await expect(agents.locator('[data-agent-id="parallel-beta"]')).toContainText('completed')
    await expect(agents.locator('[data-agent-id="failed-child"]')).toContainText('failed')
    await expect(agents.locator('[data-agent-id="nested-grandchild"]'))
      .toHaveAttribute('data-parent-id', 'parent-child')

    await agents.locator('[data-agent-id="nested-grandchild"]').click()
    await expect(conversation).toContainText('GRANDCHILD RESULT')
    await expect(page.getByLabel('Message nested-grandchild')).toBeDisabled()
    await ensureAgentsPanelOpen(page)
    await agents.getByRole('button', { name: 'All', exact: true }).click()
    await agents.locator('[data-agent-id="failed-child"]').click()
    await expect(conversation).toContainText('scripted child failure')

    const requests = await desktop.llm!.requests()
    expect(requests).toHaveLength(12)
    expect(requests.every((request) => request.matched)).toBe(true)
    await expect.poll(() => desktop.llm!.state()).toMatchObject({
      served: 12, remaining: 0, verified: true,
    })
  } finally {
    await closeDesktop(desktop)
  }
})
