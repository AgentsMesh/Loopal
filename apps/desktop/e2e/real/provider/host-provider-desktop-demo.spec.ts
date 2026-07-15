import { expect, test } from '@playwright/test'
import { closeDesktop, launchDesktop } from '../../support/electron/electron-fixture'
import { ready } from '../../support/runtime/llm-e2e-helpers'

test('manual demo completes its work and returns to conversation', async () => {
  const desktop = await launchDesktop('real', 'desktop-demo')
  try {
    const page = desktop.page
    await ready(page)
    await page.getByLabel('Message Loopal').fill('Show the production Desktop path')
    await page.getByRole('button', { name: 'Send' }).click()

    const conversation = page.getByTestId('conversation')
    await expect(conversation.getByRole('heading', {
      name: 'Loopal Desktop is connected',
    })).toBeVisible({ timeout: 20_000 })
    await ready(page)

    await expect(conversation.getByRole('list')).toContainText(
      'The conversation remains the primary surface.',
    )
    await expect(conversation.locator('blockquote')).toContainText(
      'without an automatic continuation loop',
    )
    await expect(conversation.locator('pre[data-language="sh"]')).toContainText(
      'bazel test //apps/desktop:unit',
    )
    await expect(conversation).not.toContainText('Degeneration detected')
    await expect(page.getByTestId('session-panel-zone')).toHaveCount(0)
    await expect.poll(() => desktop.llm!.state()).toMatchObject({
      name: 'desktop-demo', served: 4, requestCount: 4, remaining: 0,
      unmatchedRequests: 0, inFlight: 0, verified: true,
    })
  } finally {
    await closeDesktop(desktop)
  }
})
