import { expect, test, type Locator, type Page } from '@playwright/test'
import { readFile } from 'node:fs/promises'
import { join } from 'node:path'
import { closeDesktop, launchDesktop } from '../../support/electron/electron-fixture'
import { ready, send } from '../../support/runtime/llm-e2e-helpers'
import { type E2eProvider } from '../../support/providers/provider-e2e-fixture'
import { selectSettingsSection } from '../../support/settings/settings-helpers'

interface SettingsCase {
  provider: 'anthropic' | 'google'
  initialProvider: E2eProvider
  scenario: string
  label: 'Anthropic' | 'Google'
  model: string
  protocol: string
  prompt: string
  response: string
}

const cases: readonly SettingsCase[] = [
  {
    provider: 'anthropic', initialProvider: 'openai', scenario: 'provider-settings-anthropic',
    label: 'Anthropic', model: 'claude-opus-4-8', protocol: 'anthropic',
    prompt: 'Use Anthropic saved in Desktop Settings',
    response: 'Desktop Settings activated the authenticated Anthropic provider.',
  },
  {
    provider: 'google', initialProvider: 'anthropic', scenario: 'provider-settings-google',
    label: 'Google', model: 'gemini-2.0-flash', protocol: 'google',
    prompt: 'Use Google saved in Desktop Settings',
    response: 'Desktop Settings activated the authenticated Google provider.',
  },
]

for (const item of cases) {
  test(`Settings activates, redacts, and clears ${item.label}`, async () => {
    const desktop = await launchDesktop('real', item.scenario, {}, item.initialProvider)
    try {
      const page = desktop.page
      await ready(page)
      const pane = await openSettings(page)
      await pane.getByLabel('Default model').fill(item.model)
      await selectSettingsSection(page, 'providers')
      const card = pane.getByTestId(`provider-${item.provider}`)
      await card.getByLabel(`Enable ${item.label}`).check()
      await card.getByLabel(`${item.label} base URL`).fill(desktop.llm!.baseUrl)
      await card.getByLabel(`${item.label} API key environment`).fill('')
      await card.getByLabel(`${item.label} API key`, { exact: true }).fill(desktop.llm!.apiKey)
      await pane.getByRole('button', { name: 'Save provider settings' }).click()
      await expect(pane.getByRole('status')).toContainText('new or restarted Sessions')

      const saved = await settings(page)
      expect(saved.settings.model).toBe(item.model)
      expect(saved.configuredProviders).toContain(item.provider)
      expect(saved.providers[item.provider]).toMatchObject({
        enabled: true, baseUrl: desktop.llm!.baseUrl, apiKeyEnv: '', apiKeyConfigured: true,
      })
      expect(saved.resolvedEntries).toContainEqual({
        key: `providers.${item.provider}.api_key`, value: '********',
      })
      expect(JSON.stringify(saved)).not.toContain(desktop.llm!.apiKey)

      await pane.getByRole('button', { name: 'Restart current Session' }).click()
      await expect(pane.getByRole('status')).toContainText('restarted with the saved')
      await page.getByRole('button', { name: 'Close settings' }).click()
      await ready(page)
      await send(page, item.prompt)
      await expect(page.getByTestId('conversation')).toContainText(item.response, {
        timeout: 20_000,
      })
      await ready(page)

      expect(await desktop.llm!.requests()).toEqual([expect.objectContaining({
        protocol: item.protocol, model: item.model, apiKeyPresent: true, matched: true,
      })])
      await expect.poll(() => desktop.llm!.state()).toMatchObject({
        served: 1, remaining: 0, unmatchedRequests: 0, verified: true,
      })

      const clearPane = await openSettings(page)
      await selectSettingsSection(page, 'providers')
      const clearCard = clearPane.getByTestId(`provider-${item.provider}`)
      await clearCard.getByRole('button', { name: 'Clear API key' }).click()
      await clearPane.getByRole('button', { name: 'Save provider settings' }).click()
      await expect(clearPane.getByRole('status')).toContainText('new or restarted Sessions')
      const cleared = await settings(page)
      expect(cleared.providers[item.provider].apiKeyConfigured).toBe(false)
      expect(JSON.stringify(cleared)).not.toContain(desktop.llm!.apiKey)
      const global = await readFile(join(desktop.home, '.loopal', 'settings.json'), 'utf8')
      expect(global).not.toContain(desktop.llm!.apiKey)
    } finally {
      await closeDesktop(desktop)
    }
  })
}

test('Settings activates, redacts, and clears an OpenAI-compatible endpoint', async () => {
  const desktop = await launchDesktop(
    'real', 'provider-settings-openai-compatible', {}, 'anthropic',
  )
  try {
    const page = desktop.page
    await ready(page)
    const pane = await openSettings(page)
    await pane.getByLabel('Default model').fill('compat-e2e/deepseek-reasoner')
    await selectSettingsSection(page, 'providers')
    await pane.getByRole('button', { name: 'Add endpoint' }).click()
    const card = pane.getByTestId('provider-openai-compatible')
    await expect(card.getByLabel('Compatible provider name')).toHaveValue('compatible-1')
    await card.getByLabel('Compatible base URL').fill(`${desktop.llm!.baseUrl}/v1`)
    await card.getByLabel('Compatible model prefix').fill('compat-e2e/')
    await card.getByLabel('Compatible API key environment').fill('')
    await card.getByLabel('Compatible API key', { exact: true }).fill(desktop.llm!.apiKey)
    await pane.getByRole('button', { name: 'Save provider settings' }).click()
    await expect(pane.getByRole('status')).toContainText('new or restarted Sessions')

    const saved = await settings(page)
    expect(saved.configuredProviders).toContain('openai-compatible: compatible-1')
    expect(saved.openaiCompatible).toContainEqual({
      name: 'compatible-1', baseUrl: `${desktop.llm!.baseUrl}/v1`,
      modelPrefix: 'compat-e2e/', apiKeyEnv: '', apiKeyConfigured: true,
    })
    expect(JSON.stringify(saved)).not.toContain(desktop.llm!.apiKey)

    await pane.getByRole('button', { name: 'Restart current Session' }).click()
    await expect(pane.getByRole('status')).toContainText('restarted with the saved')
    await page.getByRole('button', { name: 'Close settings' }).click()
    await ready(page)
    await send(page, 'Use compatible provider saved in Desktop Settings')
    await expect(page.getByTestId('conversation')).toContainText(
      'Desktop Settings activated the authenticated compatible provider.', { timeout: 20_000 },
    )
    await ready(page)
    expect(await desktop.llm!.requests()).toEqual([expect.objectContaining({
      protocol: 'openai_compat', model: 'compat-e2e/deepseek-reasoner',
      apiKeyPresent: true, matched: true,
    })])
    await expect.poll(() => desktop.llm!.state()).toMatchObject({
      served: 1, remaining: 0, unmatchedRequests: 0, verified: true,
    })

    const clearPane = await openSettings(page)
    await selectSettingsSection(page, 'providers')
    const clearCard = clearPane.getByTestId('provider-openai-compatible')
    await clearCard.getByRole('button', { name: 'Clear API key' }).click()
    await clearPane.getByRole('button', { name: 'Save provider settings' }).click()
    await expect(clearPane.getByRole('status')).toContainText('new or restarted Sessions')
    const cleared = await settings(page)
    expect(cleared.openaiCompatible[0]!.apiKeyConfigured).toBe(false)
    expect(JSON.stringify(cleared)).not.toContain(desktop.llm!.apiKey)
    const global = await readFile(join(desktop.home, '.loopal', 'settings.json'), 'utf8')
    expect(global).not.toContain(desktop.llm!.apiKey)
  } finally {
    await closeDesktop(desktop)
  }
})

async function openSettings(page: Page): Promise<Locator> {
  await page.getByRole('button', { name: 'Settings' }).click()
  await selectSettingsSection(page, 'loopal')
  const pane = page.getByTestId('loopal-default-settings')
  await expect(pane).toBeVisible()
  await expect(pane.getByLabel('Default model')).toBeVisible()
  return pane
}

async function settings(page: Page) {
  return page.evaluate(() => window.loopalDesktop.getLoopalSettings('local-workspace'))
}
