import { expect, test, type Locator } from '@playwright/test'
import { closeDesktop, launchDesktop } from '../../support/electron/electron-fixture'
import { activeDetail, ready, runtimeTarget, send } from '../../support/runtime/llm-e2e-helpers'

test('renders server, failed, parallel, and progressive tools from the real model loop', async () => {
  const desktop = await launchDesktop('real', 'provider-tools')
  try {
    const page = desktop.page
    const conversation = page.getByTestId('conversation')
    await ready(page)
    const target = await runtimeTarget(page)
    await page.evaluate(async (value) => window.loopalDesktop.controlAgent({
      target: value, command: { type: 'permission', mode: 'bypass' },
    }), target)

    await send(page, 'Exercise server tools')
    await expect(conversation).toContainText('Server-side search blocks rendered.', {
      timeout: 20_000,
    })
    const serverTool = tool(conversation, 'web_search')
    await expect(serverTool.getByLabel('Completed')).toBeVisible()
    await serverTool.locator(':scope > summary').click()
    await expect(serverTool).toContainText('SERVER RESULT BODY')
    const serverDetail = await activeDetail(page)
    const serverAgent = serverDetail.agents.find((agent) => agent.id === 'main')
    expect(serverAgent?.telemetry).toMatchObject({ cacheCreationTokens: 5, cacheReadTokens: 7 })
    await ready(page)

    await send(page, 'Exercise failed tools')
    await expect(conversation).toContainText(
      'Both tool failures returned to the model.', { timeout: 20_000 },
    )
    const missing = tool(conversation, 'missing-file.txt')
    const unknown = tool(conversation, 'NoSuchDesktopTool')
    await expect(missing.getByLabel('Failed')).toBeVisible()
    await expect(unknown.getByLabel('Failed')).toBeVisible()
    await expect(missing).toContainText(/not found|No such file/i)
    await expect(unknown).toContainText(/unknown|not found|NoSuchDesktopTool/i)
    await ready(page)

    await send(page, 'Exercise parallel tools')
    await expect(conversation).toContainText('Parallel tool batch completed.', { timeout: 20_000 })
    await expect(tool(conversation, 'README.md').getByLabel('Completed')).toBeVisible()
    await expect(tool(conversation, 'src/main.rs').getByLabel('Completed')).toBeVisible()
    await ready(page)

    await send(page, 'Exercise tool progress')
    const bash = tool(conversation, 'MODEL TOOL START')
    await expect(bash.getByLabel('Running')).toBeVisible({ timeout: 15_000 })
    await expect(bash).toContainText('MODEL TOOL START', { timeout: 15_000 })
    await expect(bash.getByLabel('Completed')).toBeVisible({ timeout: 20_000 })
    await expect(bash).toContainText('MODEL TOOL END')
    await expect(conversation).toContainText('Long-running tool progress completed.', {
      timeout: 20_000,
    })
    await ready(page)

    await send(page, 'Exercise image tool result')
    const imageTool = tool(conversation, 'pixel.png')
    await expect(imageTool.getByLabel('Completed')).toBeVisible({ timeout: 20_000 })
    await expect(imageTool).toContainText(/Loaded image\/png \(1×1, \d+ bytes\)/)
    await expect(conversation).toContainText(
      'Image tool output reached the model request.', { timeout: 20_000 },
    )

    const requests = await desktop.llm!.requests()
    expect(requests).toHaveLength(9)
    expect(requests[0]!.serverBlockCount).toBe(0)
    expect(requests[1]!.serverBlockCount).toBe(0)
    expect(requests[2]!.toolResultIds).toEqual(expect.arrayContaining([
      'missing-read', 'unknown-tool',
    ]))
    expect(requests[2]!.toolResultErrorIds).toEqual(expect.arrayContaining([
      'missing-read', 'unknown-tool',
    ]))
    expect(requests[4]!.toolResultIds).toEqual(expect.arrayContaining([
      'parallel-readme', 'parallel-source',
    ]))
    expect(requests[6]!.toolResultIds).toContain('progress-bash')
    expect(requests[8]!.toolResultIds).toContain('read-image-result')
    expect(requests[8]!.imageBlockCount).toBe(1)
    await expect.poll(() => desktop.llm!.state()).toMatchObject({
      served: 9, remaining: 0, verified: true,
    })
  } finally {
    await closeDesktop(desktop)
  }
})

function tool(conversation: Locator, text: string): Locator {
  return conversation.getByTestId('tool-invocation').filter({ hasText: text }).last()
}
