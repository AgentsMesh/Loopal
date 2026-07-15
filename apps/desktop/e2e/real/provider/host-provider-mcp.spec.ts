import { expect, test } from '@playwright/test'
import { closeDesktop, launchDesktop } from '../../support/electron/electron-fixture'
import { fixturePath } from '../../support/fixtures/fixture-loader'
import { activeDetail, ready, runtimeTarget, send } from '../../support/runtime/llm-e2e-helpers'

test('discovers and calls a real repo-owned stdio MCP tool', async () => {
  const desktop = await launchDesktop('real', 'provider-mcp')
  try {
    const page = desktop.page
    await ready(page)
    const initial = await runtimeTarget(page)
    const definitions = await page.evaluate(async ({ command, script }) => (
      window.loopalDesktop.upsertMcpServer({
        workspaceId: 'local-workspace',
        server: {
          type: 'stdio', name: 'fixture_echo_server', command, args: [script],
          enabled: true, timeoutMs: 10_000, sharing: 'spawn-tree',
          cwdIsolation: null, secretPatches: [],
        },
      })
    ), { command: process.execPath, script: fixturePath('mcp/fixture-echo.mjs') })
    expect(definitions.servers).toContainEqual(expect.objectContaining({
      type: 'stdio', name: 'fixture_echo_server', enabled: true,
    }))

    await page.evaluate(
      (sessionId) => window.loopalDesktop.restartSession(sessionId), initial.sessionId,
    )
    await expect.poll(
      async () => (await runtimeTarget(page)).generation, { timeout: 30_000 },
    ).toBe(initial.generation + 1)
    await ready(page)
    await expect.poll(async () => {
      const detail = await activeDetail(page)
      return detail.view?.mcpServers.find((server) => server.name === 'fixture_echo_server')
    }, { timeout: 30_000 }).toMatchObject({
      status: 'connected', toolCount: 5, resourceCount: 1, promptCount: 1, errors: [],
    })

    const target = await runtimeTarget(page)
    await page.evaluate(async (value) => window.loopalDesktop.controlAgent({
      target: value, command: { type: 'permission', mode: 'bypass' },
    }), target)
    await send(page, 'Exercise real stdio MCP echo')

    const conversation = page.getByTestId('conversation')
    await expect(conversation).toContainText(
      'Real stdio MCP tool completed through the production model loop.',
      { timeout: 20_000 },
    )
    const invocation = conversation.getByTestId('tool-invocation')
      .filter({ hasText: 'fixture_echo' }).last()
    await expect(invocation.getByLabel('Completed')).toBeVisible()
    await invocation.locator(':scope > summary').click()
    await expect(invocation.locator('.tool-output'))
      .toHaveText('fixture_echo result: desktop-mcp-roundtrip')

    const requests = await desktop.llm!.requests()
    expect(requests).toHaveLength(2)
    expect(requests[0]!.toolNames).toContain('fixture_echo')
    expect(requests[1]!.toolResultIds).toContain('fixture-echo-call')
    expect(requests[1]!.toolResultErrorIds).not.toContain('fixture-echo-call')
    await expect.poll(() => desktop.llm!.state()).toMatchObject({
      name: 'provider-mcp', served: 2, remaining: 0, verified: true,
    })
  } finally {
    await closeDesktop(desktop)
  }
})
