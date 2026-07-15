import { expect, test, type Page } from '@playwright/test'
import {
  closeDesktop, launchDesktop, type DesktopFixture,
} from '../../support/electron/electron-fixture'
import { activeDetail, ready } from '../../support/runtime/llm-e2e-helpers'

test('routes slash controls locally and expands dynamic Skills through the real Hub', async () => {
  const desktop = await launchDesktop(
    'real', 'provider-command-surface', {}, 'anthropic', 'skill',
  )
  try {
    const page = desktop.page
    const input = page.getByLabel('Message Loopal')
    const conversation = page.getByTestId('conversation')
    await ready(page)
    await expectHidden(desktop)

    await input.fill('/p')
    const menu = page.getByTestId('command-menu')
    await expect(menu).toBeVisible()
    const plan = menu.locator('[data-command-name="/plan"]')
    const permission = menu.locator('[data-command-name="/permission"]')
    await expect(plan).toHaveAttribute('aria-selected', 'true')
    await expect(permission).toBeVisible()
    await input.press('ArrowDown')
    await expect(permission).toHaveAttribute('aria-selected', 'true')
    await input.press('ArrowUp')
    await expect(plan).toHaveAttribute('aria-selected', 'true')
    await input.press('Enter')
    await expect(input).toHaveValue('')
    await expect(input).toBeFocused()
    await expect.poll(() => agentField(page, 'mode')).toBe('plan')
    await expectNoLlm(desktop)

    await input.fill('/permission bypass')
    await input.press('Enter')
    await expect(input).toHaveValue('')
    await expect.poll(() => agentField(page, 'permissionMode')).toBe('bypass')
    await expectNoLlm(desktop)
    await expect(conversation).not.toContainText('/plan')
    await expect(conversation).not.toContainText('/permission bypass')

    await input.fill('/permission unrestricted')
    await input.press('Enter')
    await expect(page.getByTestId('command-error')).toBeVisible()
    await expect(input).toHaveValue('/permission unrestricted')
    await expect(input).toBeFocused()
    await expectNoLlm(desktop)

    await input.fill('COMMAND_SURFACE_NORMAL_PROMPT')
    await input.press('Enter')
    await expect(conversation).toContainText(
      'Ordinary prompt crossed the model boundary once.', { timeout: 20_000 },
    )
    await ready(page)
    await expect.poll(() => desktop.llm!.state()).toMatchObject({
      requestCount: 1, served: 1, remaining: 1, unmatchedRequests: 0,
    })

    await input.fill('/desktop')
    const skill = page.getByTestId('command-menu')
      .locator('[data-command-name="/desktop-check"]')
    await expect(skill).toBeVisible()
    await input.press('Tab')
    await expect(input).toHaveValue('/desktop-check ')
    await input.pressSequentially('alpha beta')
    await input.press('Enter')
    await expect(conversation).toContainText(
      'Dynamic Skill was expanded by the production Hub.', { timeout: 20_000 },
    )
    await ready(page)

    const requests = await desktop.llm!.requests()
    expect(requests).toHaveLength(2)
    expect(requests[0]).toMatchObject({
      lastUserText: 'COMMAND_SURFACE_NORMAL_PROMPT', matched: true,
    })
    expect(requests[1]!.lastUserText).toContain('SKILL_E2E_BODY_MARKER')
    expect(requests[1]!.lastUserText).toContain('alpha beta')
    await expect.poll(() => desktop.llm!.state()).toMatchObject({
      requestCount: 2, served: 2, remaining: 0,
      unmatchedRequests: 0, verified: true,
    })
    await expectHidden(desktop)
  } finally {
    await closeDesktop(desktop)
  }
})

async function agentField(
  page: Page, field: 'mode' | 'permissionMode',
): Promise<string | undefined> {
  const detail = await activeDetail(page)
  return detail.agents.find((agent) => agent.id === 'main')?.[field]
}

async function expectNoLlm(desktop: DesktopFixture): Promise<void> {
  await expect.poll(() => desktop.llm!.state()).toMatchObject({
    requestCount: 0, served: 0, remaining: 2, unmatchedRequests: 0,
  })
}

async function expectHidden(desktop: DesktopFixture): Promise<void> {
  expect(await desktop.app.evaluate(({ BrowserWindow }) => {
    const window = BrowserWindow.getAllWindows()[0]
    return { visible: window?.isVisible(), focused: window?.isFocused() }
  })).toEqual({ visible: false, focused: false })
}
