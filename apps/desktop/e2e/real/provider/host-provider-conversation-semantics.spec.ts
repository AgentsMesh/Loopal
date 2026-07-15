import { expect, test, type Locator } from '@playwright/test'
import {
  closeDesktop, launchDesktop, type DesktopFixture,
} from '../../support/electron/electron-fixture'
import { ready } from '../../support/runtime/llm-e2e-helpers'

const prompt = 'Render deterministic conversation semantics'

test('keeps a right-aligned conversation primary and renders safe Markdown', async () => {
  const desktop = await launchDesktop('real', 'provider-conversation-semantics')
  try {
    const page = desktop.page
    await ready(page)
    await expectHidden(desktop)
    await expect(page.getByTestId('primary-workspace'))
      .toHaveAttribute('data-workspace', 'conversation')
    await expect(page.getByTestId('inspector')).toHaveCount(0)
    await expect(page.getByTestId('session-panel-zone')).toHaveCount(0)

    await page.getByLabel('Message Loopal').fill(prompt)
    await page.getByRole('button', { name: 'Send' }).click()
    const transcript = page.getByTestId('conversation')
    const user = transcript.locator('[data-message-role="user"]').filter({ hasText: prompt })
    await expect(user).toHaveCount(1)
    const answer = transcript.locator('[data-message-role="assistant"]').filter({
      has: page.getByRole('heading', { name: 'Conversation semantics' }),
    })
    await expect(answer).toBeVisible({ timeout: 20_000 })
    await ready(page)

    await expectMarkdown(answer)
    await expectSafeLinks(answer)
    await expectTranscriptGeometry(transcript, user, answer)
    await expect(page.getByLabel('Message Loopal')).toBeInViewport()
    await expect(page.getByTestId('session-panel-zone')).toHaveCount(0)
    await expectHidden(desktop)

    await expect.poll(() => desktop.llm!.state()).toMatchObject({
      name: 'provider-conversation-semantics', served: 1, requestCount: 1,
      remaining: 0, unmatchedRequests: 0, inFlight: 0, verified: true,
    })
  } finally {
    await closeDesktop(desktop)
  }
})

async function expectMarkdown(answer: Locator): Promise<void> {
  await expect(answer.locator('h1')).toHaveText('Conversation semantics')
  await expect(answer.locator('h2')).toHaveText('Native structure')
  await expect(answer.locator('.rich-text > p strong')).toHaveText('bold detail')
  await expect(answer.locator('.rich-text > p code')).toHaveText('inline-code')
  await expect(answer.locator('blockquote strong')).toHaveText('bold')
  await expect(answer.locator('blockquote code')).toHaveText('quoted-code')
  await expect(answer.locator('ul > li')).toHaveCount(2)
  await expect(answer.locator('ol > li')).toHaveText(['Ordered first', 'Ordered second'])
  const fence = answer.locator('pre[data-language="typescript"] > code')
  await expect(fence).toContainText('<script>not executed</script>')
  await expect(answer.locator('script, iframe, object')).toHaveCount(0)
}

async function expectSafeLinks(answer: Locator): Promise<void> {
  const link = answer.getByRole('link', { name: 'HTTPS reference' })
  await expect(link).toHaveAttribute('href', 'https://example.com/secure')
  await expect(link).toHaveAttribute('target', '_blank')
  await expect(link).toHaveAttribute('rel', 'noreferrer')
  for (const label of ['Unsafe HTTP', 'Unsafe script', 'Unsafe relative']) {
    const text = answer.getByText(label, { exact: true })
    await expect(text).toBeVisible()
    expect(await text.evaluate((node) => node.closest('a') === null)).toBe(true)
  }
  await expect(answer.locator(
    'a[href^="http:"], a[href^="javascript:"], a[href^="file:"], a[href^="/"]',
  )).toHaveCount(0)
}

async function expectTranscriptGeometry(
  transcript: Locator, user: Locator, answer: Locator,
): Promise<void> {
  const [feed, userBox, answerBox] = await Promise.all([
    transcript.locator('.conversation-feed').boundingBox(),
    user.boundingBox(), answer.boundingBox(),
  ])
  expect(feed && userBox && answerBox).toBeTruthy()
  const userRight = userBox!.x + userBox!.width
  const answerRight = answerBox!.x + answerBox!.width
  const feedRight = feed!.x + feed!.width
  expect(userBox!.x).toBeGreaterThan(answerBox!.x)
  expect(userBox!.width).toBeLessThan(answerBox!.width)
  expect(Math.abs(userRight - answerRight)).toBeLessThanOrEqual(2)
  expect(Math.abs(userRight - feedRight)).toBeLessThanOrEqual(2)
}

async function expectHidden(desktop: DesktopFixture): Promise<void> {
  expect(await desktop.app.evaluate(({ BrowserWindow }) => {
    const window = BrowserWindow.getAllWindows()[0]
    return { visible: window?.isVisible(), focused: window?.isFocused() }
  })).toEqual({ visible: false, focused: false })
}
