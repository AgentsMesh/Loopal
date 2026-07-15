import { expect, test } from '@playwright/test'
import { readFile } from 'node:fs/promises'
import { join } from 'node:path'
import { closeDesktop, launchDesktop } from '../../../support/electron/electron-fixture'
import { ready, runtimeTarget, send } from '../../../support/runtime/llm-e2e-helpers'

test('runs question and permission classifier success and fallback paths', async () => {
  const desktop = await launchDesktop('real', 'attention-classifier')
  try {
    const page = desktop.page
    await ready(page)
    const target = await runtimeTarget(page)
    await page.evaluate(async (value) => {
      await window.loopalDesktop.controlAgent({
        target: value, command: { type: 'decision', mode: 'classifier' },
      })
      await window.loopalDesktop.controlAgent({
        target: value, command: { type: 'permission', mode: 'ask_any_write' },
      })
    }, target)
    const questions = page.getByTestId('questions-pane')
    const permissions = page.getByTestId('permissions-pane')
    const conversation = page.getByTestId('conversation')

    await send(page, 'Let the classifier answer the inferable question')
    await expect(questions).toContainText('Auto-answering', { timeout: 20_000 })
    await expect(conversation).toContainText('Classifier answer won the real race.', {
      timeout: 20_000,
    })
    await expect(questions).toHaveCount(0)
    await ready(page)

    await send(page, 'Make the classifier fail then ask me')
    await expect(questions).toContainText('Auto-answer unavailable', { timeout: 20_000 })
    await questions.getByRole('button', { name: /Manual/ }).click()
    await questions.getByRole('button', { name: 'Submit answers' }).click()
    await expect(conversation).toContainText(
      'Manual fallback answered after classifier failure.', { timeout: 20_000 },
    )
    await ready(page)

    await send(page, 'Classifier allow this write')
    await expect(conversation).toContainText('Permission classifier allowed the write.', {
      timeout: 20_000,
    })
    await expect(permissions).toHaveCount(0)
    await expect.poll(() => readFile(join(desktop.project, 'classifier-allowed.txt'), 'utf8'))
      .toBe('allowed\n')
    await ready(page)

    await send(page, 'Classifier deny this write')
    await expect(conversation).toContainText(
      'Permission classifier denial returned to the model.', { timeout: 20_000 },
    )
    await expect(permissions).toHaveCount(0)
    await expect(readFile(join(desktop.project, 'classifier-denied.txt'), 'utf8'))
      .rejects.toThrow()
    await ready(page)

    const requests = await desktop.llm!.requests()
    expect(requests).toHaveLength(12)
    for (const index of [1, 4]) expect(requests[index]).toMatchObject({
      messageCount: 1, toolCount: 0, maxTokens: 512,
    })
    for (const index of [7, 10]) expect(requests[index]).toMatchObject({
      messageCount: 1, toolCount: 0, maxTokens: 256,
    })
    await expect.poll(() => desktop.llm!.state()).toMatchObject({
      served: 12, remaining: 0, verified: true,
    })
  } finally {
    await closeDesktop(desktop)
  }
})
