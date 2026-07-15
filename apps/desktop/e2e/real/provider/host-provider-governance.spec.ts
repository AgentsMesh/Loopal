import { expect, test } from '@playwright/test'
import { closeDesktop, launchDesktop } from '../../support/electron/electron-fixture'
import { activeDetail, ready, send } from '../../support/runtime/llm-e2e-helpers'

test('projects degeneration and continuation-gate close and reopen events', async () => {
  const desktop = await launchDesktop('real', 'provider-governance')
  try {
    const page = desktop.page
    const conversation = page.getByTestId('conversation')
    await ready(page)
    for (const word of ['one', 'two', 'three', 'four', 'five']) {
      await send(page, `Degeneration sample ${word}`)
      await expect(conversation).toContainText('IDENTICAL DEGENERATION OUTPUT')
      await ready(page)
    }

    await expect(conversation).toContainText(
      'Degeneration detected: repeated text (5).', { timeout: 20_000 },
    )
    await expect(conversation).toContainText(
      /Automatic continuation paused: degeneration until /,
    )
    let detail = await activeDetail(page)
    expect(detail.conversation.filter((entry) => (
      entry.eventNotice && entry.text.includes('Degeneration detected')
    ))).toHaveLength(1)

    await send(page, 'Resume after degeneration')
    await expect(conversation).toContainText('Automatic continuation resumed.', {
      timeout: 20_000,
    })
    await expect(conversation).toContainText(
      'Fresh human progress reopened governance.', { timeout: 20_000 },
    )
    await ready(page)
    detail = await activeDetail(page)
    expect(detail.agents.find((agent) => agent.id === 'main')?.telemetry).toMatchObject({
      turnCount: 6,
    })
    expect(await desktop.llm!.requests()).toHaveLength(7)
    await expect.poll(() => desktop.llm!.state()).toMatchObject({
      served: 7, remaining: 0, verified: true,
    })
  } finally {
    await closeDesktop(desktop)
  }
})
