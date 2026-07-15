import { expect, test } from '@playwright/test'
import { closeDesktop, launchDesktop } from '../../support/electron/electron-fixture'
import { ready, runtimeTarget, send } from '../../support/runtime/llm-e2e-helpers'
import { type E2eProvider, providerModel } from '../../support/providers/provider-e2e-fixture'

const providers: readonly [E2eProvider, string][] = [
  ['anthropic', 'anthropic'],
  ['openai', 'openai_responses'],
  ['openai_compat', 'openai_compat'],
  ['google', 'google'],
]

for (const [provider, protocol] of providers) {
  test(`${provider} completes a production model and tool round trip`, async () => {
    const desktop = await launchDesktop('real', 'provider-matrix', {}, provider)
    try {
      const page = desktop.page
      const conversation = page.getByTestId('conversation')
      await ready(page)
      const target = await runtimeTarget(page)
      await page.evaluate(async (value) => window.loopalDesktop.controlAgent({
        target: value, command: { type: 'permission', mode: 'bypass' },
      }), target)
      await send(page, 'Exercise provider wire')
      await expect(conversation).toContainText(
        'Provider wire completed through the production runtime.', { timeout: 20_000 },
      )
      const tool = conversation.getByTestId('tool-invocation').filter({ hasText: 'README.md' })
      await expect(tool.getByLabel('Completed')).toBeVisible()
      const requests = await desktop.llm!.requests()
      expect(requests).toHaveLength(2)
      expect(requests.map((request) => request.protocol)).toEqual([protocol, protocol])
      expect(requests[0]!.model).toBe(providerModel(provider))
      if (provider !== 'google') {
        expect(requests[1]!.toolResultIds).toContain('provider-readme')
      }
      await expect.poll(() => desktop.llm!.state()).toMatchObject({
        served: 2, remaining: 0, verified: true,
      })
    } finally {
      await closeDesktop(desktop)
    }
  })
}
