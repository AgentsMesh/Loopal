import { expect, test } from '@playwright/test'
import {
  closeDesktop, launchDesktop, relaunchDesktop,
} from '../../support/electron/electron-fixture'
import { activeDetail, ready, send } from '../../support/runtime/llm-e2e-helpers'

const command = '/desktop-check alpha beta'
const marker = 'SKILL_E2E_BODY_MARKER'

test('routes and restores a real project Skill invocation', async () => {
  let desktop = await launchDesktop(
    'real', 'provider-skill', {}, 'anthropic', 'skill',
  )
  try {
    await ready(desktop.page)
    const initialElectronPid = desktop.app.process().pid
    await desktop.page.getByLabel('Message Loopal').fill(command)
    await desktop.page.getByRole('button', { name: 'Send' }).click()

    const conversation = desktop.page.getByTestId('conversation')
    await expect(conversation.getByText('Skill · /desktop-check')).toHaveCount(1, {
      timeout: 20_000,
    })
    await expect(conversation).toContainText('alpha beta')
    await expect(conversation).not.toContainText(marker)
    await expect(conversation).toContainText(
      'Skill invocation reached the production runtime.', { timeout: 20_000 },
    )
    await ready(desktop.page)
    await expectSkillDetail(desktop.page)

    desktop = await relaunchDesktop(desktop)
    await ready(desktop.page)
    expect(desktop.app.process().pid).not.toBe(initialElectronPid)
    const restored = desktop.page.getByTestId('conversation')
    await expect(restored.getByText('Skill · /desktop-check')).toHaveCount(1)
    await expect(restored).toContainText('alpha beta')
    await expect(restored).not.toContainText(marker)
    await expectSkillDetail(desktop.page)

    await send(desktop.page, 'SKILL_E2E_AFTER_RELAUNCH')
    await expect(restored).toContainText(
      'Skill history survived the Electron relaunch.', { timeout: 20_000 },
    )
    await ready(desktop.page)

    const requests = await desktop.llm!.requests()
    expect(requests).toHaveLength(2)
    expect(requests[0]).toMatchObject({
      protocol: 'anthropic', messageCount: 1, matched: true,
    })
    expect(requests[0]!.lastUserText).toContain(marker)
    expect(requests[0]!.lastUserText).toContain('alpha beta')
    expect(requests[1]).toMatchObject({
      protocol: 'anthropic', messageCount: 3,
      lastUserText: 'SKILL_E2E_AFTER_RELAUNCH', matched: true,
    })
    await expect.poll(() => desktop.llm!.state()).toMatchObject({
      served: 2, requestCount: 2, remaining: 0,
      unmatchedRequests: 0, inFlight: 0, verified: true,
    })
  } finally {
    await closeDesktop(desktop)
  }
})

async function expectSkillDetail(page: import('@playwright/test').Page): Promise<void> {
  const detail = await activeDetail(page)
  const skill = detail.conversation.find((entry) => entry.skill?.name === '/desktop-check')
  expect(skill).toMatchObject({
    role: 'user',
    skill: { name: '/desktop-check', userArgs: 'alpha beta' },
  })
  expect(skill?.text).toContain(marker)
}
