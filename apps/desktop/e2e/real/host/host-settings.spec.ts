import { expect, test } from '@playwright/test'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import {
  closeDesktop, launchDesktop, relaunchDesktop, waitForHostStatus,
} from '../../support/electron/electron-fixture'
import { selectSettingsSection } from '../../support/settings/settings-helpers'

const settings = {
  model: 'desktop-persisted-model', modelRouting: {
    default: '', summarization: '', classification: 'desktop-classifier-model', refine: '',
  }, permissionMode: 'ask_dangerous' as const,
  decisionMode: 'classifier' as const, sandboxPolicy: 'read_only' as const,
  thinking: { type: 'effort' as const, level: 'high' as const },
  maxContextTokens: 180_000, memoryEnabled: false, microcompactIdleMinutes: 15,
  telemetryEnabled: false, outputStyle: 'engineer',
}

test('persists redacted Loopal defaults globally without rewriting the workspace', async () => {
  let desktop = await launchDesktop('real')
  try {
    await waitForHostStatus(desktop.page, 'ready')
    const globalDirectory = join(desktop.home, '.loopal')
    const globalPath = join(globalDirectory, 'settings.json')
    const projectDirectory = join(desktop.project, '.loopal')
    const localPath = join(projectDirectory, 'settings.local.json')
    await Promise.all([
      mkdir(globalDirectory, { recursive: true }),
      mkdir(projectDirectory, { recursive: true }),
    ])
    await writeFile(join(projectDirectory, 'settings.json'), JSON.stringify({
      model_routing: { summarization: 'inherited-summary-model' },
    }))
    await writeFile(globalPath, JSON.stringify({
      model: 'before',
      providers: { anthropic: {
        api_key: 'existing-provider-value', unknown_provider_field: 'preserve-me',
      } },
      unknown_global: { preserve: true },
    }))
    const projectLocal = {
      sandbox: { network: { denied_domains: ['blocked.test'] } },
      output_style: 'workspace-only-style', unknown_project: { preserve: true },
    }
    await writeFile(localPath, JSON.stringify(projectLocal))

    const before = await desktop.page.evaluate(
      () => window.loopalDesktop.getLoopalSettings('local-workspace'),
    )
    expect(before.settings.model).toBe('before')
    expect(before.settings.modelRouting.summarization).toBe('')
    expect(before.settings.outputStyle).toBe('')
    expect(before.resolvedEntries).toContainEqual({
      key: 'model_routing.summarization', value: 'inherited-summary-model',
    })
    expect(before.resolvedEntries).toContainEqual({
      key: 'output_style', value: 'workspace-only-style',
    })
    expect(before.configuredProviders).toContain('anthropic')
    expect(before.providers.anthropic.apiKeyConfigured).toBe(true)
    expect(JSON.stringify(before)).not.toContain('existing-provider-value')
    expect(before.settingSources.every((source) => !source.includes(desktop.root))).toBe(true)

    const beforeRuntime = (await desktop.page.evaluate(
      () => window.loopalDesktop.bootstrap(),
    )).runtimes[0]!
    await desktop.page.getByRole('button', { name: 'Settings' }).click()
    await selectSettingsSection(desktop.page, 'loopal')
    const pane = desktop.page.getByTestId('loopal-default-settings')
    await expect(pane.getByLabel('Default model')).toHaveValue('before')
    await pane.getByLabel('Default model').fill(settings.model)
    await pane.getByLabel('Summarization override').fill('')
    await pane.getByLabel('Classification override').fill(
      settings.modelRouting.classification,
    )
    await pane.getByLabel('Default permission mode').selectOption(settings.permissionMode)
    await pane.getByLabel('Default decision mode').selectOption(settings.decisionMode)
    await pane.getByLabel('Default sandbox policy').selectOption(settings.sandboxPolicy)
    await pane.getByLabel('Default thinking').selectOption('effort:high')
    await pane.getByLabel(/Max context tokens/).fill(String(settings.maxContextTokens))
    await pane.getByLabel(/Microcompact idle minutes/).fill(
      String(settings.microcompactIdleMinutes),
    )
    await setChecked(pane.getByLabel('Enable project memory'), settings.memoryEnabled)
    await setChecked(pane.getByLabel('Enable telemetry'), settings.telemetryEnabled)
    await pane.getByLabel('Output style').fill(settings.outputStyle)
    await selectSettingsSection(desktop.page, 'providers')
    await pane.getByLabel('Enable OpenAI').check()
    await pane.getByLabel('OpenAI base URL').fill('https://proxy.example.test/v1')
    await pane.getByLabel('OpenAI API key environment').fill('LOOPAL_OPENAI_KEY')
    await pane.getByLabel('OpenAI API key', { exact: true }).fill('new-provider-write-only-value')
    await pane.getByRole('button', { name: 'Save provider settings' }).click()
    await expect(pane.getByRole('status')).toContainText('new or restarted Sessions')
    const raw = JSON.parse(await readFile(globalPath, 'utf8'))
    expect(raw.providers.anthropic.api_key).toBe('existing-provider-value')
    expect(raw.providers.anthropic.unknown_provider_field).toBe('preserve-me')
    expect(raw.providers.openai.api_key).toBe('new-provider-write-only-value')
    expect(raw.providers.openai.base_url).toBe('https://proxy.example.test/v1')
    expect(raw.providers.openai.api_key_env).toBe('LOOPAL_OPENAI_KEY')
    expect(raw.model_routing.summarization).toBeNull()
    expect(raw.model_routing.classification).toBe('desktop-classifier-model')
    expect(raw.unknown_global.preserve).toBe(true)
    expect(raw.sandbox.policy).toBe('read_only')
    expect(JSON.parse(await readFile(localPath, 'utf8'))).toEqual(projectLocal)

    const reread = await desktop.page.evaluate(
      () => window.loopalDesktop.getLoopalSettings('local-workspace'),
    )
    expect(reread.settings).toEqual(settings)
    expect(reread.providers.openai).toEqual({
      enabled: true, baseUrl: 'https://proxy.example.test/v1',
      apiKeyEnv: 'LOOPAL_OPENAI_KEY', apiKeyConfigured: true,
    })
    expect(JSON.stringify(reread)).not.toContain('existing-provider-value')
    expect(JSON.stringify(reread)).not.toContain('new-provider-write-only-value')
    expect(reread.resolvedEntries.find((entry) =>
      entry.key === 'providers.openai.api_key')?.value).toBe('********')
    expect(reread.resolvedEntries.find((entry) =>
      entry.key === 'providers.openai.api_key_env')?.value).toBe('********')
    expect(reread.resolvedEntries).toContainEqual({
      key: 'model_routing.summarization', value: 'inherited-summary-model',
    })
    await selectSettingsSection(desktop.page, 'loopal')
    await pane.getByText('Advanced resolved config').click()
    await pane.getByLabel('Search resolved config').fill('providers.openai.api_key')
    await expect(pane.getByRole('cell', { name: '********' }).first()).toBeVisible()
    await desktop.page.getByRole('button', { name: 'Close settings' }).click()
    desktop = await relaunchDesktop(desktop)
    await waitForHostStatus(desktop.page, 'ready')
    await desktop.page.getByRole('button', { name: 'Settings' }).click()
    await selectSettingsSection(desktop.page, 'loopal')
    const reloaded = desktop.page.getByTestId('loopal-default-settings')
    await expect(reloaded.getByLabel('Default model')).toHaveValue(settings.model)
    await expect(reloaded.getByLabel('Summarization override')).toHaveValue('')
    await expect(reloaded.getByLabel('Output style')).toHaveValue(settings.outputStyle)
    expect(JSON.parse(await readFile(localPath, 'utf8'))).toEqual(projectLocal)
    await selectSettingsSection(desktop.page, 'providers')
    await reloaded.getByTestId('provider-openai')
      .getByRole('button', { name: 'Clear API key' }).click()
    await reloaded.getByRole('button', { name: 'Save provider settings' }).click()
    await expect(reloaded.getByRole('status')).toContainText('new or restarted Sessions')
    const clearedRaw = JSON.parse(await readFile(globalPath, 'utf8'))
    expect(clearedRaw.providers.openai.api_key).toBeNull()
    expect(clearedRaw.providers.anthropic.api_key).toBe('existing-provider-value')
    const cleared = await desktop.page.evaluate(
      () => window.loopalDesktop.getLoopalSettings('local-workspace'),
    )
    expect(cleared.providers.openai.apiKeyConfigured).toBe(false)
    expect(JSON.stringify(cleared)).not.toContain('existing-provider-value')
    await reloaded.getByRole('button', { name: 'Restart current Session' }).click()
    await expect(reloaded.getByRole('status')).toContainText('restarted with the saved')
    const afterRuntime = (await desktop.page.evaluate(
      () => window.loopalDesktop.bootstrap(),
    )).runtimes.find((runtime) => runtime.sessionId === beforeRuntime.sessionId)!
    expect(afterRuntime.generation).toBeGreaterThan(beforeRuntime.generation)
    await expect.poll(async () => desktop.page.evaluate(async (sessionId) => {
      const detail = await window.loopalDesktop.openSession(sessionId)
      return detail.agents.find((agent) => !agent.parentId)?.model
    }, beforeRuntime.sessionId), { timeout: 30_000 }).toBe(settings.model)
    expect(JSON.parse(await readFile(localPath, 'utf8'))).toEqual(projectLocal)
    await expect(desktop.page.evaluate(
      () => window.loopalDesktop.getLoopalSettings('outside-workspace'),
    )).rejects.toThrow(/Unknown workspace|unknown workspace/)
  } finally {
    await closeDesktop(desktop)
  }
})

async function setChecked(locator: import('@playwright/test').Locator, checked: boolean) {
  if (await locator.isChecked() !== checked) await locator.click()
}
