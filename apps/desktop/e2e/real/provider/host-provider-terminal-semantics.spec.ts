import { expect, test } from '@playwright/test'
import { closeDesktop, launchDesktop } from '../../support/electron/electron-fixture'
import { activeDetail, ready, send } from '../../support/runtime/llm-e2e-helpers'
import { type E2eProvider } from '../../support/providers/provider-e2e-fixture'

const terminals: readonly [E2eProvider, string][] = [
  ['anthropic', 'anthropic'], ['openai', 'openai_responses'],
  ['openai_compat', 'openai_compat'], ['google', 'google'],
]

for (const [provider, protocol] of terminals) {
  test(`${provider} maps its max-token terminal into one continuation`, async () => {
    const desktop = await launchDesktop('real', 'provider-terminal-max', {}, provider)
    try {
      const page = desktop.page
      const conversation = page.getByTestId('conversation')
      await ready(page)
      await send(page, 'Exercise provider max token terminal')
      await expect(conversation).toContainText(
        'Provider-specific max token continuation completed.', { timeout: 20_000 },
      )
      await expect(conversation).toContainText(
        'Output truncated (max_tokens). Auto-continuing (1/3)',
      )
      await expect(conversation.locator('[data-message-role="user"]')).toHaveCount(1)
      await ready(page)
      const requests = await desktop.llm!.requests()
      expect(requests).toHaveLength(2)
      expect(requests.map((request) => request.protocol)).toEqual([protocol, protocol])
      expect(requests[1]!.lastUserText).toBe(provider === 'anthropic'
        ? '[Continue from where you left off]' : 'Exercise provider max token terminal')
      expect((await activeDetail(page)).agents.find((agent) => agent.id === 'main')
        ?.telemetry?.turnCount).toBe(1)
      await expect.poll(() => desktop.llm!.state()).toMatchObject({
        served: 2, remaining: 0, verified: true,
      })
    } finally {
      await closeDesktop(desktop)
    }
  })
}

test('OpenAI response.failed is fatal, redacted, and recoverable next turn', async () => {
  const desktop = await launchDesktop('real', 'provider-openai-failed', {}, 'openai')
  try {
    const page = desktop.page
    const conversation = page.getByTestId('conversation')
    await ready(page)
    await send(page, 'Trigger OpenAI failed')
    await expect(conversation).toContainText('openai API request failed', { timeout: 20_000 })
    await expect(conversation).not.toContainText('sensitive upstream detail')
    await expect(conversation).not.toContainText('Auto-continuing')
    await send(page, 'Recover from OpenAI failed')
    await expect(conversation).toContainText(
      'OpenAI Session recovered after a structured terminal failure.', { timeout: 20_000 },
    )
    await expect.poll(() => desktop.llm!.state()).toMatchObject({
      served: 2, remaining: 0, verified: true,
    })
  } finally {
    await closeDesktop(desktop)
  }
})

test('Google SAFETY terminates normally without a truncation warning', async () => {
  const desktop = await launchDesktop('real', 'provider-google-safety', {}, 'google')
  try {
    const conversation = desktop.page.getByTestId('conversation')
    await ready(desktop.page)
    await send(desktop.page, 'Exercise Google safety terminal')
    await expect(conversation).toContainText('Google safety terminal completed.', {
      timeout: 20_000,
    })
    await expect(conversation).not.toContainText('network interruption')
    await expect(conversation).not.toContainText('Auto-continuing')
    await ready(desktop.page)
    await expect.poll(() => desktop.llm!.state()).toMatchObject({
      served: 1, remaining: 0, verified: true,
    })
  } finally {
    await closeDesktop(desktop)
  }
})
