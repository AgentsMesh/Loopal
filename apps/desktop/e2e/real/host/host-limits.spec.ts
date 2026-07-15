import { expect, test } from '@playwright/test'
import { writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import {
  closeDesktop, launchDesktop, waitForHostStatus,
} from '../../support/electron/electron-fixture'

test('bounds workspace responses without losing the Desktop Host', async () => {
  test.setTimeout(90_000)
  const desktop = await launchDesktop('real')
  try {
    await waitForHostStatus(desktop.page, 'ready')
    const workspaceId = await desktop.page.evaluate(async () => (
      (await window.loopalDesktop.bootstrap()).workspaces[0]!.id
    ))
    const large = join(desktop.project, 'large.txt')

    await writeFile(large, 'x'.repeat(10_000_000))
    await expect(desktop.page.evaluate(async ({ id }) => {
      const document = await window.loopalDesktop.readFile({ workspaceId: id, path: 'large.txt' })
      return document.content.length
    }, { id: workspaceId })).resolves.toBe(10_000_000)

    await writeFile(large, 'x'.repeat(10_000_001))
    await expect(desktop.page.evaluate(
      (id) => window.loopalDesktop.readFile({ workspaceId: id, path: 'large.txt' }),
      workspaceId,
    )).rejects.toThrow(/file_too_large|10 MB/i)

    await writeFile(large, `${'界'.repeat(2_000)}needle\n`)
    const search = await desktop.page.evaluate(
      (id) => window.loopalDesktop.searchWorkspace({ workspaceId: id, query: 'needle' }),
      workspaceId,
    )
    expect(search.truncated).toBe(true)
    expect(new TextEncoder().encode(search.matches[0]!.preview).length).toBeLessThanOrEqual(4_000)

    await writeFile(join(desktop.project, 'README.md'), `${'changed\n'.repeat(1_200_000)}`)
    await expect(desktop.page.evaluate(
      (id) => window.loopalDesktop.gitDiff({ workspaceId: id, path: 'README.md' }),
      workspaceId,
    )).rejects.toThrow(/response_too_large|response limit|8 MiB/i)

    const alive = await desktop.page.evaluate(
      (id) => window.loopalDesktop.listDirectory({ workspaceId: id, path: '' }),
      workspaceId,
    )
    expect(alive.entries.some((entry) => entry.name === 'README.md')).toBe(true)
    await waitForHostStatus(desktop.page, 'ready')
  } finally {
    await closeDesktop(desktop)
  }
})
