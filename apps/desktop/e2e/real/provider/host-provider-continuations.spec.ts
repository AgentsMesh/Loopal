import { expect, test, type Locator, type Page } from '@playwright/test'
import { readFile } from 'node:fs/promises'
import { join } from 'node:path'
import { closeDesktop, launchDesktop } from '../../support/electron/electron-fixture'
import { activeDetail, ready, send } from '../../support/runtime/llm-e2e-helpers'

test('projects every continuation reason and enforces the continuation cap', async () => {
  const desktop = await launchDesktop('real', 'provider-continuations')
  try {
    const page = desktop.page
    const conversation = page.getByTestId('conversation')
    await ready(page)

    await send(page, 'Exercise pause continuation')
    await expect(conversation).toContainText('Pause continuation completed.', { timeout: 20_000 })
    await expect(autoContinuations(page).last()).toContainText(
      'Provider paused the turn. Auto-continuing (1/3)',
    )
    await ready(page)

    await send(page, 'Discard max token tool')
    await expect(conversation).toContainText(
      'Max-token tool was discarded safely.', { timeout: 20_000 },
    )
    await expect(autoContinuations(page).last()).toContainText(
      'Output truncated during tool calls (max_tokens); incomplete tools discarded. '
      + 'Auto-continuing (1/3)',
    )
    await expect(readFile(join(desktop.project, 'must-not-exist-max.txt'), 'utf8'))
      .rejects.toThrow()
    await ready(page)
    await expect(discardedTool(conversation, 'must-not-exist-max.txt').getByLabel('Stale'))
      .toBeVisible()
    await expect(discardedTool(conversation, 'must-not-exist-max.txt').getByLabel('Queued'))
      .toHaveCount(0)
    await expect(discardedTool(conversation, 'must-not-exist-max.txt').getByLabel('Running'))
      .toHaveCount(0)

    await send(page, 'Recover dropped stream')
    await expect(conversation).toContainText(
      'Response stream ended unexpectedly', { timeout: 20_000 },
    )
    await expect(conversation).toContainText(
      'Dropped stream continued without running its tool.', { timeout: 20_000 },
    )
    await expect(autoContinuations(page).last()).toContainText(
      'Response stream ended unexpectedly. Auto-continuing (1/3)',
    )
    await expect(readFile(join(desktop.project, 'must-not-exist-stream.txt'), 'utf8'))
      .rejects.toThrow()
    await ready(page)
    await expect(discardedTool(conversation, 'must-not-exist-stream.txt').getByLabel('Stale'))
      .toBeVisible()
    await expect(discardedTool(conversation, 'must-not-exist-stream.txt').getByLabel('Queued'))
      .toHaveCount(0)
    await expect(discardedTool(conversation, 'must-not-exist-stream.txt').getByLabel('Running'))
      .toHaveCount(0)

    await send(page, 'Reach continuation cap')
    await expect(conversation).toContainText('CAP SEGMENT FOUR', { timeout: 20_000 })
    await ready(page)

    const requests = await desktop.llm!.requests()
    expect(requests).toHaveLength(10)
    expect(requests[1]!.lastUserText).toBe('[Continue from where you left off]')
    expect(requests[3]!.assistantBlockTypes).not.toContain('tool_use')
    expect(requests[3]!.toolResultIds).not.toContain('max-tool-write')
    expect(requests[5]!.assistantBlockTypes).not.toContain('tool_use')
    expect(requests[5]!.toolResultIds).not.toContain('dropped-tool-write')
    expect(requests.slice(6)).toHaveLength(4)
    const detail = await activeDetail(page)
    expect(detail.conversation.filter((entry) => entry.role === 'user')).toHaveLength(4)
    expect(detail.agents.find((agent) => agent.id === 'main')?.telemetry?.turnCount).toBe(4)
    await expect.poll(() => desktop.llm!.state()).toMatchObject({
      served: 10, remaining: 0, unmatchedRequests: 0, verified: true,
    })
  } finally {
    await closeDesktop(desktop)
  }
})

function autoContinuations(page: Page) {
  return page.locator('[data-message-role="system"]').filter({ hasText: 'Auto-continuing' })
}

function discardedTool(conversation: Locator, marker: string) {
  return conversation.getByTestId('tool-invocation').filter({ hasText: marker }).last()
}
