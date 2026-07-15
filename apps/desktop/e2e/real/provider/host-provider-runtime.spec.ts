import { expect, test } from '@playwright/test'
import { readFile } from 'node:fs/promises'
import { join } from 'node:path'
import { closeDesktop, launchDesktop } from '../../support/electron/electron-fixture'
import { ready, send } from '../../support/runtime/llm-e2e-helpers'
import { selectSettingsSection } from '../../support/settings/settings-helpers'

test('retries a real Anthropic HTTP 429 and clears the retry state', async () => {
  const desktop = await launchDesktop('real', 'provider-retry')
  try {
    const page = desktop.page
    await ready(page)
    await send(page, 'Recover from a scripted rate limit')
    await expect(page.getByTestId('conversation')).toContainText(
      'Retrying in 2.0s', { timeout: 10_000 },
    )
    await expect(page.getByTestId('conversation')).toContainText(
      'Recovered through the real provider retry loop.', { timeout: 15_000 },
    )
    await expect(page.getByTestId('conversation')).not.toContainText('Retrying in 2.0s')
    expect(await desktop.llm!.requests()).toHaveLength(2)
    await expect.poll(() => desktop.llm!.state()).toMatchObject({
      served: 2, remaining: 0, verified: true,
    })
  } finally {
    await closeDesktop(desktop)
  }
})

test('continues a real max_tokens response without duplicating the user turn', async () => {
  const desktop = await launchDesktop('real', 'provider-max-tokens')
  try {
    const page = desktop.page
    await ready(page)
    await send(page, 'Exercise max token continuation')
    const conversation = page.getByTestId('conversation')
    await expect(conversation).toContainText('PARTIAL MAX TOKEN OUTPUT')
    await expect(conversation).toContainText('Output truncated (max_tokens). Auto-continuing')
    await expect(conversation).toContainText(
      'Continuation completed over a second HTTP request.', { timeout: 15_000 },
    )
    await expect(conversation.locator('[data-message-role="user"]')).toHaveCount(1)
    const requests = await desktop.llm!.requests()
    expect(requests).toHaveLength(2)
    expect(requests[1]!.lastUserText).toBe('[Continue from where you left off]')
    await expect.poll(() => desktop.llm!.state()).toMatchObject({ verified: true })
  } finally {
    await closeDesktop(desktop)
  }
})

test('interrupts a live HTTP stream and keeps the Session reusable', async () => {
  const desktop = await launchDesktop('real', 'provider-interrupt')
  try {
    const page = desktop.page
    await ready(page)
    await send(page, 'Hold this provider stream')
    const conversation = page.getByTestId('conversation')
    await expect(conversation).toContainText('STREAM START BEFORE INTERRUPT')
    await page.getByRole('button', { name: 'Settings' }).click()
    await selectSettingsSection(page, 'agent')
    await page.getByRole('group', { name: 'Agent controls' })
      .getByRole('button', { name: 'Interrupt' }).click()
    await page.getByRole('button', { name: 'Close settings' }).click()
    await expect(conversation).toContainText('Turn cancelled', { timeout: 15_000 })
    await expect.poll(() => desktop.llm!.state()).toMatchObject({ clientDisconnects: 1 })
    await expect(conversation).not.toContainText('LATE CHUNK MUST NOT RENDER')
    await send(page, 'Recover after interrupt')
    await expect(conversation).toContainText(
      'Recovered after cancelling the real HTTP stream.', { timeout: 15_000 },
    )
    await expect(page.getByLabel('Message Loopal')).toBeEnabled()
    await expect.poll(() => desktop.llm!.state()).toMatchObject({
      served: 2, remaining: 0, verified: true,
    })
  } finally {
    await closeDesktop(desktop)
  }
})

test('retries a connection closed before HTTP headers', async () => {
  const desktop = await launchDesktop('real', 'provider-preheader-retry')
  try {
    const conversation = desktop.page.getByTestId('conversation')
    await ready(desktop.page)
    await send(desktop.page, 'Recover from a pre-header disconnect')
    await expect(conversation).toContainText('Retrying in 2.0s', { timeout: 10_000 })
    await expect(conversation).toContainText(
      'Recovered after the transport disappeared before headers.', { timeout: 15_000 },
    )
    await expect.poll(() => desktop.llm!.state()).toMatchObject({
      served: 2, remaining: 0, scriptedDisconnects: 1, verified: true,
    })
  } finally {
    await closeDesktop(desktop)
  }
})

test('interrupts a retry wait and accepts the next turn immediately', async () => {
  const desktop = await launchDesktop('real', 'provider-retry-interrupt')
  try {
    const page = desktop.page
    const conversation = page.getByTestId('conversation')
    await ready(page)
    await send(page, 'Wait on a long provider retry')
    await expect(conversation).toContainText('Retrying in 30.0s', { timeout: 10_000 })
    await page.getByRole('button', { name: 'Settings' }).click()
    await selectSettingsSection(page, 'agent')
    await page.getByRole('group', { name: 'Agent controls' })
      .getByRole('button', { name: 'Interrupt' }).click()
    await page.getByRole('button', { name: 'Close settings' }).click()
    await expect(conversation).not.toContainText('Retrying in 30.0s', { timeout: 10_000 })
    await expect(page.getByLabel('Message Loopal')).toBeEnabled({ timeout: 10_000 })
    await send(page, 'Recover after interrupting retry wait')
    await expect(conversation).toContainText(
      'Recovered without waiting for the cancelled backoff.', { timeout: 15_000 },
    )
    await expect.poll(() => desktop.llm!.state()).toMatchObject({
      served: 2, remaining: 0, verified: true,
    })
  } finally {
    await closeDesktop(desktop)
  }
})

test('cancels a running model tool and suppresses late process output', async () => {
  const desktop = await launchDesktop('real', 'provider-tool-interrupt')
  try {
    const page = desktop.page
    const conversation = page.getByTestId('conversation')
    await ready(page)
    const target = await page.evaluate(async () => {
      const bootstrap = await window.loopalDesktop.bootstrap()
      const sessionId = bootstrap.activeSessionId!
      const runtime = bootstrap.runtimes.find((item) => item.sessionId === sessionId)!
      return { sessionId, runtimeId: runtime.id, generation: runtime.generation,
        agentId: runtime.rootAgent }
    })
    await page.evaluate(async (value) => window.loopalDesktop.controlAgent({
      target: value, command: { type: 'permission', mode: 'bypass' },
    }), target)
    await send(page, 'Start an interruptible model tool')
    const tool = conversation.getByTestId('tool-invocation').filter({ hasText: 'TOOL EARLY' })
    await expect(tool.getByLabel('Running')).toBeVisible({ timeout: 15_000 })
    const progress = tool.locator('.tool-progress')
    await expect(progress).toContainText('TOOL EARLY')
    await page.getByRole('button', { name: 'Settings' }).click()
    await selectSettingsSection(page, 'agent')
    await page.getByRole('group', { name: 'Agent controls' })
      .getByRole('button', { name: 'Interrupt' }).click()
    await page.getByRole('button', { name: 'Close settings' }).click()
    await expect(tool.getByLabel('Cancelled')).toBeVisible({ timeout: 15_000 })
    await expect(progress).toHaveCount(0)
    await page.waitForTimeout(3_500)
    await expect(readFile(join(desktop.project, 'tool-late-marker.txt'), 'utf8'))
      .rejects.toThrow()
    await send(page, 'Recover after cancelling a model tool')
    await expect(conversation).toContainText(
      'The Session remained reusable after tool cancellation.', { timeout: 20_000 },
    )
    await expect.poll(() => desktop.llm!.state()).toMatchObject({
      served: 2, remaining: 0, verified: true,
    })
    const requests = await desktop.llm!.requests()
    expect(requests[1]!.toolResultIds).toContain('interrupt-bash')
    expect(requests[1]!.toolResultErrorIds).toContain('interrupt-bash')
  } finally {
    await closeDesktop(desktop)
  }
})
