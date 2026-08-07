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

test('Google SAFETY preserves partial output, fails closed, and recovers next turn', async () => {
  const desktop = await launchDesktop('real', 'provider-google-safety', {}, 'google')
  try {
    const page = desktop.page
    const conversation = page.getByTestId('conversation')
    await ready(page)
    await send(page, 'Exercise Google safety terminal')
    await expect(conversation).toContainText('GOOGLE SAFETY PARTIAL OUTPUT', {
      timeout: 20_000,
    })
    await expect(conversation.locator('[data-message-role="error"]')).toContainText(
      'status=400, message=google candidate terminated: SAFETY', { timeout: 20_000 },
    )
    await expect(page.getByTestId('runtime-status')).toContainText('Failed')
    await expect(conversation).not.toContainText('network interruption')
    await expect(conversation).not.toContainText('Auto-continuing')

    const failed = await activeDetail(page)
    expect(failed.session.attention).toBe('failure')
    expect(failed.agents.find((agent) => agent.id === 'main')?.status).toBe('failed')
    expect(failed.conversation.some((entry) => (
      entry.role === 'error'
        && entry.text.includes('google candidate terminated: SAFETY')
    ))).toBe(true)
    expect(failed.view?.goal?.status).not.toBe('complete')

    await send(page, 'Recover after Google safety terminal')
    await expect(conversation).toContainText(
      'Google Session recovered after a safety terminal error.', { timeout: 20_000 },
    )
    await ready(page)
    const recovered = await activeDetail(page)
    expect(recovered.session.attention).not.toBe('failure')
    expect(recovered.agents.find((agent) => agent.id === 'main')?.status).not.toBe('failed')
    await expect.poll(() => desktop.llm!.state()).toMatchObject({
      served: 2, remaining: 0, unmatchedRequests: 0, verified: true,
    })
  } finally {
    await closeDesktop(desktop)
  }
})
