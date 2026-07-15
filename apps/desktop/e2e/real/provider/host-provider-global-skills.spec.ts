import { expect, test, type Page } from '@playwright/test'
import { readFile, stat } from 'node:fs/promises'
import { join } from 'node:path'
import {
  closeDesktop, launchDesktop, relaunchDesktop, type DesktopFixture,
} from '../../support/electron/electron-fixture'
import { seedPlugin } from '../../support/fixtures/fixture-loader'
import { activeDetail, ready } from '../../support/runtime/llm-e2e-helpers'
import { selectSettingsSection } from '../../support/settings/settings-helpers'

const name = '/global-e2e'
const command = `${name} alpha beta`
const initialDescription = 'Run the initial global Skill contract'
const editedDescription = 'Run the edited global Skill contract'
const initialBody = 'GLOBAL_SKILL_INITIAL_BODY_MARKER\nUse $ARGUMENTS.'
const editedBody = 'GLOBAL_SKILL_EDITED_BODY_MARKER\nUse $ARGUMENTS.'

test('manages, invokes, and restores global Skills and plugin contributions', async () => {
  let desktop = await launchDesktop('real', 'provider-global-skills')
  const skillPath = join(desktop.home, '.loopal', 'skills', 'global-e2e.md')
  try {
    await ready(desktop.page)
    await expectHidden(desktop)
    await openSkills(desktop.page)
    const section = desktop.page.getByTestId('skills-plugin-settings')

    await section.getByTestId('skill-create').click()
    await section.getByTestId('skill-name').fill(name)
    await section.getByTestId('skill-description').fill(initialDescription)
    await section.getByTestId('skill-body').fill(initialBody)
    await section.getByTestId('skill-save').click()
    await expect(section.getByRole('status')).toContainText(`Saved ${name}`)
    await expect(section.getByTestId('skill-description')).toHaveValue(initialDescription)
    await expectFile(skillPath, initialDescription, initialBody)

    await section.getByTestId('skill-description').fill(editedDescription)
    await section.getByTestId('skill-body').fill(editedBody)
    await section.getByTestId('skill-save').click()
    await expectFile(skillPath, editedDescription, editedBody)
    await expect.poll(() => readFile(skillPath, 'utf8')).not.toContain(initialBody)
    await section.getByRole('button', { name: 'Cancel' }).click()
    await expect(section.getByTestId('global-skill-global-e2e'))
      .toContainText(editedDescription)

    await desktop.page.getByRole('button', { name: 'Close settings' }).click()
    await desktop.page.getByLabel('Message Loopal').fill(command)
    await desktop.page.getByRole('button', { name: 'Send' }).click()
    const conversation = desktop.page.getByTestId('conversation')
    await expect(conversation.getByText(`Skill · ${name}`)).toHaveCount(1, {
      timeout: 20_000,
    })
    await expect(conversation).toContainText('alpha beta')
    await expect(conversation).not.toContainText('GLOBAL_SKILL_EDITED_BODY_MARKER')
    await expect(conversation).toContainText(
      'Global Skill invocation reached the production runtime.', { timeout: 20_000 },
    )
    await ready(desktop.page)
    const detail = await activeDetail(desktop.page)
    expect(detail.conversation.find((entry) => entry.skill?.name === name)).toMatchObject({
      role: 'user', skill: { name, userArgs: 'alpha beta' },
    })
    const requests = await desktop.llm!.requests()
    expect(requests).toHaveLength(1)
    expect(requests[0]).toMatchObject({
      protocol: 'anthropic', messageCount: 1, matched: true,
    })
    expect(requests[0]!.lastUserText).toContain(editedBody.split('\n')[0])
    expect(requests[0]!.lastUserText).toContain('alpha beta')

    await seedPlugin(desktop.home, 'global-skills')
    desktop = await relaunchDesktop(desktop)
    await ready(desktop.page)
    await expectHidden(desktop)
    await openSkills(desktop.page)
    const restored = desktop.page.getByTestId('skills-plugin-settings')
    const global = restored.getByTestId('global-skill-global-e2e')
    await expect(global).toContainText(editedDescription)
    await expect(restored.getByTestId('effective-skill-list')).toContainText('/plugin-check')
    await expect(restored.getByTestId('effective-skill-list')).toContainText(name)
    const pluginCard = restored.getByTestId('plugin-global-skills')
    await expect(pluginCard).toContainText('/plugin-check')
    await expect(pluginCard).toContainText('fixture-mcp')
    await expect(pluginCard).toContainText('LOOPAL.md')
    await expectPluginContract(desktop.page)
    await global.getByRole('button', { name: `Edit ${name}` }).click()
    await expect(restored.getByTestId('skill-description')).toHaveValue(editedDescription)
    await expect(restored.getByTestId('skill-body')).toHaveValue(editedBody)

    await restored.getByTestId('skill-delete').click()
    const confirmation = restored.getByRole('alertdialog')
    await confirmation.getByRole('button', { name: 'Delete permanently' }).click()
    await expect(restored.getByTestId('global-skill-global-e2e')).toHaveCount(0)
    await expect.poll(() => exists(skillPath)).toBe(false)
    await expect.poll(() => desktop.llm!.state()).toMatchObject({
      served: 1, requestCount: 1, remaining: 0,
      unmatchedRequests: 0, inFlight: 0, verified: true,
    })
  } finally {
    await closeDesktop(desktop)
  }
})

async function openSkills(page: Page): Promise<void> {
  await page.getByRole('button', { name: 'Settings' }).click()
  await selectSettingsSection(page, 'skills')
  await expect(page.getByTestId('skills-plugin-settings')).toBeVisible()
}

async function expectFile(path: string, ...parts: string[]): Promise<void> {
  for (const part of parts) {
    await expect.poll(() => readFile(path, 'utf8')).toContain(part)
  }
}

async function exists(path: string): Promise<boolean> {
  try { await stat(path); return true } catch { return false }
}

async function expectHidden(desktop: DesktopFixture): Promise<void> {
  expect(await desktop.app.evaluate(({ BrowserWindow }) => {
    const window = BrowserWindow.getAllWindows()[0]
    return { visible: window?.isVisible(), focused: window?.isFocused() }
  })).toEqual({ visible: false, focused: false })
}

async function expectPluginContract(page: Page): Promise<void> {
  const response = await page.evaluate(async () => {
    const bootstrap = await window.loopalDesktop.bootstrap()
    return window.loopalDesktop.listPlugins(bootstrap.workspaces[0]!.id)
  })
  expect(response.plugins).toContainEqual({
    name: 'global-skills', source: 'plugin:global-skills', skills: ['/plugin-check'],
    mcpServers: ['fixture-mcp'], hookCount: 0,
    hasSettings: true, hasInstructions: true, hasMemory: false,
  })
}
