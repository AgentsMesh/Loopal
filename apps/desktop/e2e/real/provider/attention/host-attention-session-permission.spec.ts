import { expect, test } from '@playwright/test'
import { readFile } from 'node:fs/promises'
import { join } from 'node:path'
import { closeDesktop, launchDesktop } from '../../../support/electron/electron-fixture'
import { ready, runtimeTarget, send } from '../../../support/runtime/llm-e2e-helpers'

test('persists an allow-for-session decision across model tool calls', async () => {
  const desktop = await launchDesktop('real', 'attention-session-permission')
  try {
    const page = desktop.page
    await ready(page)
    const target = await runtimeTarget(page)
    await page.evaluate(async (value) => {
      await window.loopalDesktop.controlAgent({
        target: value, command: { type: 'decision', mode: 'manual' },
      })
      await window.loopalDesktop.controlAgent({
        target: value, command: { type: 'permission', mode: 'ask_any_write' },
      })
    }, target)
    const approvals = page.getByTestId('permissions-pane')
    const conversation = page.getByTestId('conversation')

    await send(page, 'Grant a session write')
    await expect(approvals).toContainText('Allow Write')
    await approvals.getByRole('button', { name: 'Allow for session' }).click()
    await expect(conversation).toContainText(
      'Session permission stored after the first write.', { timeout: 20_000 },
    )
    await expect.poll(() => readFile(join(desktop.project, 'session-one.txt'), 'utf8'))
      .toBe('one\n')
    await ready(page)

    await send(page, 'Reuse the session write grant')
    await expect(conversation).toContainText(
      'Second write reused the session permission.', { timeout: 20_000 },
    )
    await expect(approvals).toHaveCount(0)
    await expect.poll(() => readFile(join(desktop.project, 'session-two.txt'), 'utf8'))
      .toBe('two\n')
    await ready(page)
    await expect.poll(() => desktop.llm!.state()).toMatchObject({
      served: 4, remaining: 0, verified: true,
    })
  } finally {
    await closeDesktop(desktop)
  }
})
