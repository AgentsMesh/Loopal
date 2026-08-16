import { expect, test } from '@playwright/test'
import { readFile } from 'node:fs/promises'
import { join } from 'node:path'
import { closeDesktop, launchDesktop } from '../../support/electron/electron-fixture'
import { fixturePath } from '../../support/fixtures/fixture-loader'
import { activeDetail, ready, runtimeTarget, send } from '../../support/runtime/llm-e2e-helpers'
import { selectSettingsSection } from '../../support/settings/settings-helpers'

test('preserves rich MCP, error, reconnect, and cancellation semantics', async () => {
  const desktop = await launchDesktop('real', 'provider-mcp-rich')
  try {
    const page = desktop.page
    const initial = await runtimeTarget(page)
    await ready(page)
    await page.evaluate(async ({ command, script }) => window.loopalDesktop.upsertMcpServer({
      workspaceId: 'local-workspace',
      server: {
        type: 'stdio', name: 'fixture_echo_server', command, args: [script],
        enabled: true, timeoutMs: 10_000, sharing: 'spawn-tree',
        cwdIsolation: null, secretPatches: [],
      },
    }), { command: process.execPath, script: fixturePath('mcp/fixture-echo.mjs') })
    await page.evaluate(
      (sessionId) => window.loopalDesktop.restartSession(sessionId), initial.sessionId,
    )
    await expect.poll(
      async () => (await runtimeTarget(page)).generation, { timeout: 30_000 },
    ).toBe(initial.generation + 1)
    await ready(page)
    await expect.poll(async () => mcpSnapshot(page), { timeout: 30_000 }).toMatchObject({
      status: 'connected', toolCount: 5, resourceCount: 1, promptCount: 1, errors: [],
    })
    await page.getByRole('tab', { name: 'MCP', exact: true }).click()
    await expect(page.getByTestId('mcp-runtime-pane')).toContainText(
      '5 tools · 1 resources · 1 prompts',
    )
    await page.waitForTimeout(2_000)
    const target = await runtimeTarget(page)
    await page.evaluate(async (value) => window.loopalDesktop.controlAgent({
      target: value, command: { type: 'permission', mode: 'bypass' },
    }), target)

    await send(page, 'Exercise rich MCP content')
    const conversation = page.getByTestId('conversation')
    await expect(conversation).toContainText('Rich MCP content reached the model.', {
      timeout: 20_000,
    })
    const rich = conversation.getByTestId('tool-invocation').filter({ hasText: 'fixture_rich' })
    await expect(rich.getByLabel('Completed')).toBeVisible()
    await rich.locator(':scope > summary').click()
    const output = rich.locator('.tool-output')
    await expect(output).toContainText('fixture rich text')
    await expect(output).toContainText('[MCP binary content denied]')
    await expect(output).not.toContainText('data:image/png;base64,iVBORw0KGgo=')
    await expect(output).toContainText('[resource fixture://embedded]')
    await expect(output).toContainText('[resource: fixture://linked]')
    await ready(page)

    await send(page, 'Exercise MCP error result')
    await expect(conversation).toContainText('MCP error result was preserved for the model.', {
      timeout: 20_000,
    })
    const failed = conversation.getByTestId('tool-invocation').filter({ hasText: 'fixture_error' })
    await expect(failed.getByLabel('Failed')).toBeVisible()
    await expect(failed.locator('.tool-detail')).toContainText('fixture MCP error result')
    await ready(page)

    await send(page, 'Exercise MCP transport reconnect')
    await expect(conversation).toContainText(
      'MCP transport reconnected and retried exactly once.', { timeout: 20_000 },
    )
    await expect(readFile(join(desktop.root, 'mcp-reconnect-marker.txt'), 'utf8'))
      .resolves.toBe('closed once\n')
    const reconnected = conversation.getByTestId('tool-invocation')
      .filter({ hasText: 'fixture_reconnect' })
    await expect(reconnected.getByLabel('Completed')).toBeVisible()
    await ready(page)

    await send(page, 'Start cancellable MCP tool')
    const slow = conversation.getByTestId('tool-invocation').filter({ hasText: 'fixture_slow' })
    await expect(slow.getByLabel('Running')).toBeVisible({ timeout: 15_000 })
    await page.getByRole('button', { name: 'Settings' }).click()
    await selectSettingsSection(page, 'agent')
    await page.getByRole('group', { name: 'Agent controls' })
      .getByRole('button', { name: 'Interrupt' }).click()
    await page.getByRole('button', { name: 'Close settings' }).click()
    await expect(slow.getByLabel('Cancelled')).toBeVisible({ timeout: 15_000 })
    await page.waitForTimeout(4_500)
    await expect(conversation).not.toContainText('fixture slow late result')
    await send(page, 'Recover after cancelling MCP tool')
    await expect(conversation).toContainText(
      'Session remained reusable after MCP cancellation.', { timeout: 20_000 },
    )
    await ready(page)

    const requests = await desktop.llm!.requests()
    expect(requests).toHaveLength(8)
    expect(requests[0]!.toolNames).toEqual(expect.arrayContaining([
      'fixture_rich', 'fixture_error', 'fixture_reconnect', 'fixture_slow',
    ]))
    expect(requests[3]!.toolResultErrorIds).toContain('mcp-error-call')
    expect(requests[7]!.toolResultErrorIds).toContain('mcp-slow-call')
    await expect.poll(() => desktop.llm!.state()).toMatchObject({
      served: 8, remaining: 0, unmatchedRequests: 0, inFlight: 0, verified: true,
    })
  } finally {
    await closeDesktop(desktop)
  }
})

async function mcpSnapshot(page: import('@playwright/test').Page) {
  const detail = await activeDetail(page)
  return detail.view?.mcpServers.find((server) => server.name === 'fixture_echo_server')
}
