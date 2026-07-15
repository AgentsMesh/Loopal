import { expect, test } from '@playwright/test'
import {
  launchDesktop, shutdownDesktop, waitForHostStatus,
} from '../../support/electron/electron-fixture'
import {
  createSessionDirectory, queueSessionDirectories,
} from '../../support/fixtures/session-directory-fixture'
import { createFromDirectory } from '../../support/fixtures/session-directory-ui'
import {
  activeSessionId, processIsRunning, sendMarker, sessionCard,
  shutdownAndAssertClean, waitForHosts, waitForSessionHost,
} from '../../support/runtime/host-process'

test('isolates two live Hosts and replaces only the restarted Session runtime', async () => {
  const desktop = await launchDesktop('real')
  const observedPids = new Set<number>()
  try {
    await waitForHostStatus(desktop.page, 'ready')
    const firstId = await activeSessionId(desktop.page)
    const firstHost = (await waitForHosts(desktop.home, 1))[0]!
    observedPids.add(firstHost.pid)
    expect(firstHost.root_session_id).toBe(firstId)
    await sendMarker(desktop.page, 'session A marker')

    const secondDirectory = await createSessionDirectory(desktop, 'second-host', false)
    await queueSessionDirectories(desktop, [secondDirectory.path])
    await createFromDirectory(desktop.page, 'directory')
    await expect(desktop.page.locator('[data-session-id]')).toHaveCount(2)
    const secondId = await activeSessionId(desktop.page)
    expect(secondId).not.toBe(firstId)
    const twoHosts = await waitForHosts(desktop.home, 2)
    twoHosts.forEach(({ pid }) => observedPids.add(pid))
    expect(new Set(twoHosts.map((item) => item.root_session_id))).toEqual(
      new Set([firstId, secondId]),
    )
    await sendMarker(desktop.page, 'session B marker')

    await sessionCard(desktop.page, firstId).click()
    await expect(desktop.page.getByTestId('conversation')).toContainText('session A marker')
    await expect(desktop.page.getByTestId('conversation')).not.toContainText('session B marker')
    await desktop.page.evaluate((sessionId) => (
      window.loopalDesktop.stopSession(sessionId)
    ), firstId)
    const remaining = await waitForHosts(desktop.home, 1)
    expect(remaining[0]!.root_session_id).toBe(secondId)
    await expect.poll(() => processIsRunning(firstHost.pid), { timeout: 10_000 }).toBe(false)
    expect(processIsRunning(remaining[0]!.pid)).toBe(true)

    await desktop.page.evaluate((sessionId) => (
      window.loopalDesktop.restartSession(sessionId)
    ), firstId)
    const restarted = await waitForSessionHost(desktop.home, firstId, firstHost.pid)
    observedPids.add(restarted.pid)
    expect(restarted.pid).not.toBe(firstHost.pid)
    await expect(sessionCard(desktop.page, firstId).locator('small')).toHaveText(
      'Waiting', { timeout: 30_000 },
    )
    await expect(desktop.page.getByTestId('conversation')).toContainText('session A marker')
    await sessionCard(desktop.page, secondId).click()
    await expect(desktop.page.getByTestId('conversation')).toContainText('session B marker')
    await expect(desktop.page.getByTestId('conversation')).not.toContainText('session A marker')
    await shutdownAndAssertClean(desktop, observedPids)
  } finally {
    await shutdownDesktop(desktop).catch(() => undefined)
    await desktop.cleanup()
  }
})
