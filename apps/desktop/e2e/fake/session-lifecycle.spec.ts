import { expect, test } from '@playwright/test'
import {
  closeDesktop, launchDesktop, waitForHostStatus,
} from '../support/electron/electron-fixture'
import { queueSessionDirectories } from '../support/fixtures/session-directory-fixture'

test('creates, stops, and restarts an isolated fake session generation', async () => {
  const desktop = await launchDesktop('fake')
  try {
    const page = desktop.page
    await waitForHostStatus(page, 'ready')
    const current = page.getByTestId('current-session-list')
    await expect(current.locator('.session-card')).toHaveCount(2)
    await expect(page.getByTestId('history-session-list')).toHaveCount(0)
    await expect(page.getByLabel('Active workspace')).toHaveCount(0)
    await expect(page.getByLabel('Active session')).toHaveCount(0)

    await queueSessionDirectories(desktop, [desktop.project])
    await page.locator('.new-session').click()
    const dialog = page.getByTestId('new-session-dialog')
    await expect(dialog).toBeVisible()
    await dialog.getByTestId('session-directory').click()
    await expect(dialog.getByTestId('create-session-confirm')).toBeEnabled()
    await dialog.getByTestId('create-session-confirm').click()
    await expect(dialog).toHaveCount(0)
    await expect(current.locator('.session-card')).toHaveCount(3)

    const created = await page.evaluate(async () => (await window.loopalDesktop.bootstrap())
      .sessions.find(({ id }) => id.startsWith('session-1'))!)
    const visibleIds = await current.locator('.session-card').evaluateAll((cards) => (
      cards.map((card) => card.getAttribute('data-session-id'))
    ))
    const workspaceIds = await page.evaluate(async (ids) => new Set(
      (await window.loopalDesktop.bootstrap()).sessions
        .filter(({ id }) => ids.includes(id)).map(({ workspaceId }) => workspaceId),
    ).size, visibleIds)
    expect(workspaceIds).toBeGreaterThanOrEqual(2)

    const result = await page.evaluate(async (createdId) => {
      await window.loopalDesktop.stopSession(createdId)
      const stopped = await window.loopalDesktop.openSession(createdId)
      const second = await window.loopalDesktop.restartSession(createdId)
      const third = await window.loopalDesktop.restartSession(createdId)
      const restarted = await window.loopalDesktop.openSession(createdId)
      return {
        createdId,
        stoppedStatus: stopped.session.status,
        stoppedRuntime: stopped.session.activeRuntimeId,
        generations: [second.generation, third.generation],
        restartedRuntime: restarted.session.activeRuntimeId,
        thirdRuntime: third.id,
      }
    }, created.id)
    expect(result.createdId).toMatch(/^session-/)
    expect(result.stoppedStatus).toBe('stopped')
    expect(result.stoppedRuntime).toBeUndefined()
    expect(result.generations).toEqual([2, 3])
    expect(result.restartedRuntime).toBe(result.thirdRuntime)
    const windowState = await desktop.app.evaluate(({ BrowserWindow }) => {
      const current = BrowserWindow.getAllWindows()[0]
      return { visible: current?.isVisible(), focused: current?.isFocused() }
    })
    expect(windowState).toEqual({ visible: false, focused: false })
  } finally {
    await closeDesktop(desktop)
  }
})
