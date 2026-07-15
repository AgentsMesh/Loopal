import { expect, test } from '@playwright/test'
import { closeDesktop, launchDesktop } from '../../support/electron/electron-fixture'
import { activeDetail, ready, send } from '../../support/runtime/llm-e2e-helpers'
import { selectSettingsSection } from '../../support/settings/settings-helpers'

test('runs manual model summarization and uses the compacted boundary next turn', async () => {
  const desktop = await launchDesktop('real', 'provider-compaction')
  try {
    const page = desktop.page
    const conversation = page.getByTestId('conversation')
    await ready(page)
    for (const message of ['Manual compact seed one', 'Manual compact seed two']) {
      await send(page, message)
      await ready(page)
    }

    await page.getByRole('button', { name: 'Settings' }).click()
    await selectSettingsSection(page, 'agent')
    const controls = page.getByRole('group', { name: 'Agent controls' })
    await controls.getByLabel('Compact instructions').fill('Preserve contract markers')
    await controls.getByRole('button', { name: 'Compact', exact: true }).click()
    await page.getByRole('button', { name: 'Close settings' }).click()

    await expect(page.getByTestId('runtime-status')).toContainText('Compacting', {
      timeout: 15_000,
    })
    await expect(conversation.locator('.conversation-banner')).toContainText(
      'summarizing context',
    )
    await expect(conversation).toContainText('Context compacted (manual)', { timeout: 30_000 })
    await expect(conversation.locator('.conversation-banner')).toHaveCount(0)
    await ready(page)

    await send(page, 'Use the compacted context now')
    await expect(conversation).toContainText(
      'Post-compaction request used the summary boundary.', { timeout: 20_000 },
    )
    await ready(page)

    const requests = await desktop.llm!.requests()
    expect(requests).toHaveLength(4)
    expect(requests[2]!.hasSystem).toBe(true)
    expect(requests[3]!.lastUserText).toContain('Use the compacted context now')
    const detail = await activeDetail(page)
    expect(detail.view?.compactBanner).toBeNull()
    expect(detail.conversation.some((entry) => (
      entry.role === 'system' && entry.text.includes('Context compacted (manual)')
    ))).toBe(true)
    await expect.poll(() => desktop.llm!.state()).toMatchObject({
      served: 4, remaining: 0, verified: true,
    })
  } finally {
    await closeDesktop(desktop)
  }
})
