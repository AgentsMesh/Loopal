import { expect, test } from '@playwright/test'
import { closeDesktop, launchDesktop } from '../../support/electron/electron-fixture'
import { ready, send } from '../../support/runtime/llm-e2e-helpers'
import { selectSettingsSection } from '../../support/settings/settings-helpers'

const model = 'gpt-4.1'
const prompt = 'Use the provider saved in Desktop Settings'

test('saves an OpenAI provider in Settings and uses it after restart', async () => {
  const desktop = await launchDesktop('real', 'provider-settings-openai')
  try {
    const page = desktop.page
    await ready(page)
    await page.getByRole('button', { name: 'Settings' }).click()
    await selectSettingsSection(page, 'loopal')
    const pane = page.getByTestId('loopal-default-settings')
    await pane.getByLabel('Default model').fill(model)
    await selectSettingsSection(page, 'providers')
    await pane.getByLabel('Enable OpenAI').check()
    await pane.getByLabel('OpenAI base URL').fill(`${desktop.llm!.baseUrl}/v1`)
    await pane.getByLabel('OpenAI API key environment').fill('')
    await pane.getByLabel('OpenAI API key', { exact: true }).fill(desktop.llm!.apiKey)
    await pane.getByRole('button', { name: 'Save provider settings' }).click()
    await expect(pane.getByRole('status')).toContainText('new or restarted Sessions')

    const settings = await page.evaluate(
      () => window.loopalDesktop.getLoopalSettings('local-workspace'),
    )
    expect(settings.settings.model).toBe(model)
    expect(settings.providers.openai).toMatchObject({
      enabled: true, baseUrl: `${desktop.llm!.baseUrl}/v1`,
      apiKeyEnv: '', apiKeyConfigured: true,
    })
    expect(JSON.stringify(settings)).not.toContain(desktop.llm!.apiKey)

    const restart = pane.getByRole('button', { name: 'Restart current Session' })
    await expect(restart).toBeEnabled()
    await restart.click()
    await expect(pane.getByRole('status')).toContainText('restarted with the saved')
    await page.getByRole('button', { name: 'Close settings' }).click()
    await ready(page)
    await send(page, prompt)
    await expect(page.getByTestId('conversation')).toContainText(
      'Desktop Settings selected the OpenAI model.', { timeout: 20_000 },
    )
    await ready(page)

    expect(await desktop.llm!.requests()).toEqual([expect.objectContaining({
      sequence: 1, protocol: 'openai_responses', model,
      lastUserText: prompt, messageCount: 1, apiKeyPresent: true,
      protocolVersionPresent: true, stream: true, matched: true,
    })])
    await expect.poll(() => desktop.llm!.state()).toMatchObject({
      served: 1, requestCount: 1, remaining: 0,
      unmatchedRequests: 0, verified: true,
    })
  } finally {
    await closeDesktop(desktop)
  }
})
