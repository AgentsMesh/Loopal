import { expect, test } from '@playwright/test'
import { closeDesktop, launchDesktop } from '../../../support/electron/electron-fixture'
import { ready, runtimeTarget, send } from '../../../support/runtime/llm-e2e-helpers'

test('returns Other, multi-select, and cancellation answers to the real model loop', async () => {
  const desktop = await launchDesktop('real', 'attention-answers')
  try {
    const page = desktop.page
    await ready(page)
    const target = await runtimeTarget(page)
    await page.evaluate(async (value) => window.loopalDesktop.controlAgent({
      target: value, command: { type: 'decision', mode: 'manual' },
    }), target)
    const questions = page.getByTestId('questions-pane')
    const conversation = page.getByTestId('conversation')

    await send(page, 'Ask for a custom answer')
    await expect(questions).toContainText('Describe the verification style')
    await questions.getByRole('textbox', {
      name: /Other answer for .*Describe the verification style/,
    }).fill('custom fixture answer')
    await questions.getByRole('button', { name: 'Submit answers' }).click()
    await expect(conversation).toContainText(
      'Custom free-text answer reached the model.', { timeout: 20_000 },
    )
    await expect(questions).toHaveCount(0)
    await ready(page)

    await send(page, 'Ask multiple questions')
    await expect(questions).toContainText('Pick several checks')
    await questions.getByRole('button', { name: /Build/ }).click()
    await questions.getByRole('button', { name: /Tests/ }).click()
    await questions.getByRole('textbox', {
      name: /Other answer for .*Pick several checks/,
    }).fill('custom multi check')
    await questions.getByRole('button', { name: /Deep/ }).click()
    await questions.getByRole('button', { name: 'Submit answers' }).click()
    await expect(conversation).toContainText(
      'Multi-select and custom answers reached the model.', { timeout: 20_000 },
    )
    await expect(questions).toHaveCount(0)
    await ready(page)

    await send(page, 'Ask a cancellable question')
    await expect(questions).toContainText('Cancel this verification request')
    await questions.getByRole('button', { name: 'Cancel', exact: true }).click()
    await expect(conversation).toContainText(
      'Question cancellation reached the model.', { timeout: 20_000 },
    )
    await expect(questions).toHaveCount(0)
    await ready(page)

    const requests = await desktop.llm!.requests()
    expect(requests).toHaveLength(6)
    expect(requests[1]!.toolResultIds).toContain('ask-custom')
    expect(requests[3]!.toolResultIds).toContain('ask-multiple')
    expect(requests[5]!.toolResultIds).toContain('ask-cancel')
    await expect.poll(() => desktop.llm!.state()).toMatchObject({
      served: 6, remaining: 0, verified: true,
    })
  } finally {
    await closeDesktop(desktop)
  }
})
