import { expect, test } from '@playwright/test'
import { readFile } from 'node:fs/promises'
import { join } from 'node:path'
import { closeDesktop, launchDesktop } from '../../support/electron/electron-fixture'
import { ready, runtimeTarget, send } from '../../support/runtime/llm-e2e-helpers'
import { selectSettingsSection } from '../../support/settings/settings-helpers'

test('OpenAI retries a transport closed before response headers', async () => {
  const desktop = await launchDesktop('real', 'provider-openai-preheader', {}, 'openai')
  try {
    const page = desktop.page
    const conversation = page.getByTestId('conversation')
    await ready(page)
    await send(page, 'Recover OpenAI before headers')
    await expect(conversation).toContainText('Retrying in 2.0s', { timeout: 10_000 })
    await expect(conversation).toContainText(
      'OpenAI recovered after its pre-header transport failure.', { timeout: 20_000 },
    )
    await expect(conversation).not.toContainText('Retrying in 2.0s')
    expect((await desktop.llm!.requests()).map((request) => request.protocol))
      .toEqual(['openai_responses', 'openai_responses'])
    await expect.poll(() => desktop.llm!.state()).toMatchObject({
      served: 2, remaining: 0, scriptedDisconnects: 1,
      unmatchedRequests: 0, verified: true,
    })
  } finally {
    await closeDesktop(desktop)
  }
})

test('Google retains partial output and auto-continues a malformed stream', async () => {
  const desktop = await launchDesktop('real', 'provider-google-partial', {}, 'google')
  try {
    const page = desktop.page
    const conversation = page.getByTestId('conversation')
    await ready(page)
    await send(page, 'Recover a partial Google stream')
    await expect(conversation).toContainText('GOOGLE PARTIAL STREAM MARKER', {
      timeout: 15_000,
    })
    await expect(conversation).toContainText('SSE parse error', { timeout: 15_000 })
    await expect(conversation).toContainText(
      'Response stream ended unexpectedly. Auto-continuing (1/3)',
    )
    await expect(conversation).toContainText(
      'Google recovered its partial stream through continuation.', { timeout: 20_000 },
    )
    await ready(page)
    const requests = await desktop.llm!.requests()
    expect(requests).toHaveLength(2)
    expect(requests.every((request) => request.protocol === 'google')).toBe(true)
    await expect.poll(() => desktop.llm!.state()).toMatchObject({
      served: 2, remaining: 0, unmatchedRequests: 0, verified: true,
    })
  } finally {
    await closeDesktop(desktop)
  }
})

test('OpenAI-compatible returns failed and cancelled tools to the model', async () => {
  const desktop = await launchDesktop(
    'real', 'provider-compat-tool-semantics', {}, 'openai_compat',
  )
  try {
    const page = desktop.page
    const conversation = page.getByTestId('conversation')
    await ready(page)
    const target = await runtimeTarget(page)
    await page.evaluate(async (value) => window.loopalDesktop.controlAgent({
      target: value, command: { type: 'permission', mode: 'bypass' },
    }), target)

    await send(page, 'Exercise compatible tool failure')
    const failed = conversation.getByTestId('tool-invocation')
      .filter({ hasText: 'missing-compat-provider.txt' })
    await expect(failed.getByLabel('Failed')).toBeVisible({ timeout: 15_000 })
    await expect(conversation).toContainText(
      'Compatible provider observed the failed tool result.', { timeout: 20_000 },
    )
    await ready(page)

    await send(page, 'Start a compatible cancellable tool')
    const cancelled = conversation.getByTestId('tool-invocation')
      .filter({ hasText: 'COMPAT TOOL EARLY' })
    await expect(cancelled.getByLabel('Running')).toBeVisible({ timeout: 15_000 })
    await expect(cancelled).toContainText('COMPAT TOOL EARLY')
    await page.getByRole('button', { name: 'Settings' }).click()
    await selectSettingsSection(page, 'agent')
    await page.getByRole('group', { name: 'Agent controls' })
      .getByRole('button', { name: 'Interrupt' }).click()
    await page.getByRole('button', { name: 'Close settings' }).click()
    await expect(cancelled.getByLabel('Cancelled')).toBeVisible({ timeout: 15_000 })
    await page.waitForTimeout(3_500)
    await expect(readFile(join(desktop.project, 'compat-late.txt'), 'utf8')).rejects.toThrow()

    await send(page, 'Recover compatible tool cancellation')
    await expect(conversation).toContainText(
      'Compatible provider retained the cancelled tool result.', { timeout: 20_000 },
    )
    const requests = await desktop.llm!.requests()
    expect(requests).toHaveLength(4)
    expect(requests.every((request) => request.protocol === 'openai_compat')).toBe(true)
    expect(requests[1]!.toolResultIds).toContain('compat-missing-read')
    expect(requests[1]!.assistantBlockTypes).toEqual(['thinking', 'tool_use'])
    expect(requests[3]!.toolResultIds).toContain('compat-cancel-bash')
    await expect.poll(() => desktop.llm!.state()).toMatchObject({
      served: 4, remaining: 0, unmatchedRequests: 0, verified: true,
    })
  } finally {
    await closeDesktop(desktop)
  }
})
