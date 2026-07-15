import { expect, test } from '@playwright/test'
import { basename } from 'node:path'
import {
  launchDesktop, shutdownDesktop, waitForHostStatus,
} from '../../support/electron/electron-fixture'
import {
  listHosts, processIsRunning, sendMarker, waitForHosts,
} from '../../support/runtime/host-process'

test('runs the bundled Loopal CLI through handshake, Hub RPC, and shutdown', async () => {
  const desktop = await launchDesktop('real')
  let hostPid = 0
  try {
    await waitForHostStatus(desktop.page, 'ready')
    await expect(desktop.page.getByTestId('active-session-title')).toContainText(
      basename(desktop.project),
    )
    hostPid = (await waitForHosts(desktop.home, 1))[0]!.pid
    await sendMarker(desktop.page, 'Reply with ok')
    await shutdownDesktop(desktop)
    await expect.poll(() => processIsRunning(hostPid), { timeout: 10_000 }).toBe(false)
    await expect.poll(() => listHosts(desktop.home).then((items) => items.length)).toBe(0)
  } finally {
    await shutdownDesktop(desktop).catch(() => undefined)
    await desktop.cleanup()
  }
})
