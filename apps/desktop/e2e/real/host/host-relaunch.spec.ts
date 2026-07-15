import { expect, test } from '@playwright/test'
import { realpath } from 'node:fs/promises'
import {
  launchDesktop, relaunchDesktop, shutdownDesktop, waitForHostStatus,
} from '../../support/electron/electron-fixture'
import {
  activeSessionId, processIsRunning, sendMarker, sessionCard,
  shutdownAndAssertClean, storedSession, waitForHosts, waitForSessionHost,
} from '../../support/runtime/host-process'

test('restores a persisted Loopal Session after the Electron app relaunches', async () => {
  let desktop = await launchDesktop('real')
  const observedPids = new Set<number>()
  try {
    await waitForHostStatus(desktop.page, 'ready')
    const persistedId = await activeSessionId(desktop.page)
    const original = (await waitForHosts(desktop.home, 1))[0]!
    observedPids.add(original.pid)
    await sendMarker(desktop.page, 'persisted session marker')
    const canonicalProject = await realpath(desktop.project)
    await expect.poll(() => storedSession(desktop.home, persistedId)).toMatchObject({
      id: persistedId, cwd: canonicalProject,
    })

    desktop = await relaunchDesktop(desktop)
    await expect.poll(() => processIsRunning(original.pid), { timeout: 10_000 }).toBe(false)
    await waitForHostStatus(desktop.page, 'ready')
    await expect.poll(() => storedSession(desktop.home, persistedId)).toMatchObject({
      id: persistedId,
    })
    const freshHosts = await waitForHosts(desktop.home, 1)
    freshHosts.forEach(({ pid }) => observedPids.add(pid))
    await expect(sessionCard(desktop.page, persistedId)).toBeVisible()
    await sessionCard(desktop.page, persistedId).click()
    await expect(desktop.page.getByTestId('conversation')).toContainText(
      'persisted session marker', { timeout: 30_000 },
    )
    const restored = await waitForSessionHost(desktop.home, persistedId, original.pid)
    observedPids.add(restored.pid)
    expect(restored.pid).not.toBe(original.pid)
    await shutdownAndAssertClean(desktop, observedPids)
  } finally {
    await shutdownDesktop(desktop).catch(() => undefined)
    await desktop.cleanup()
  }
})
