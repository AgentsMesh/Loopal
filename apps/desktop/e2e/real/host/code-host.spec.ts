import { expect, test } from '@playwright/test'
import { writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import {
  closeDesktop, launchDesktop, waitForHostStatus,
} from '../../support/electron/electron-fixture'

test('runs real workspace, Git, worktree, and watch services', async () => {
  const desktop = await launchDesktop('real')
  try {
    await waitForHostStatus(desktop.page, 'ready')
    const context = await desktop.page.evaluate(async () => {
      const bootstrap = await window.loopalDesktop.bootstrap()
      return {
        workspaceId: bootstrap.workspaces[0]!.id,
      }
    })
    const workspaceId = context.workspaceId

    const listing = await desktop.page.evaluate(
      (id) => window.loopalDesktop.listDirectory({ workspaceId: id, path: '' }),
      workspaceId,
    )
    expect(listing.entries.map((entry) => entry.name)).toEqual(
      expect.arrayContaining(['README.md', 'src']),
    )

    const initial = await readMain(desktop.page, workspaceId)
    await writeFile(join(desktop.project, 'src', 'main.rs'), 'fn main() { println!("external"); }\n')
    await expect(desktop.page.evaluate(
      ({ id, version }) => window.loopalDesktop.writeFile({
        workspaceId: id,
        path: 'src/main.rs',
        content: 'stale\n',
        expectedVersion: version,
      }),
      { id: workspaceId, version: initial.version },
    )).rejects.toThrow(/expected/i)

    const current = await readMain(desktop.page, workspaceId)
    const saved = await desktop.page.evaluate(
      ({ id, version }) => window.loopalDesktop.writeFile({
        workspaceId: id,
        path: 'src/main.rs',
        content: 'fn main() { println!("saved"); }\n',
        expectedVersion: version,
      }),
      { id: workspaceId, version: current.version },
    )
    expect(saved.content).toContain('saved')

    const search = await desktop.page.evaluate(
      (id) => window.loopalDesktop.searchWorkspace({ workspaceId: id, query: 'saved' }),
      workspaceId,
    )
    expect(search.matches[0]).toMatchObject({ path: 'src/main.rs', line: 1 })
    const diff = await desktop.page.evaluate(
      (id) => window.loopalDesktop.gitDiff({ workspaceId: id, path: 'src/main.rs' }),
      workspaceId,
    )
    expect(diff.original).toContain('ready')
    expect(diff.modified).toContain('saved')

    await desktop.page.evaluate(
      (id) => window.loopalDesktop.gitStage({ workspaceId: id, path: 'src/main.rs' }),
      workspaceId,
    )
    let status = await desktop.page.evaluate(
      (id) => window.loopalDesktop.gitStatus(id),
      workspaceId,
    )
    expect(status.changes.find((item) => item.path === 'src/main.rs')?.indexStatus).toBe('M')
    await desktop.page.evaluate(
      (id) => window.loopalDesktop.gitUnstage({ workspaceId: id, path: 'src/main.rs' }),
      workspaceId,
    )
    status = await desktop.page.evaluate((id) => window.loopalDesktop.gitStatus(id), workspaceId)
    expect(status.changes.find((item) => item.path === 'src/main.rs')?.worktreeStatus).toBe('M')

    const worktree = await desktop.page.evaluate(
      (id) => window.loopalDesktop.createWorktree({ workspaceId: id, name: 'desktop-e2e' }),
      workspaceId,
    )
    expect(worktree).toMatchObject({ id: 'desktop-e2e', isMain: false, hasChanges: false })
    const worktrees = await desktop.page.evaluate(
      (id) => window.loopalDesktop.listWorktrees(id),
      workspaceId,
    )
    expect(worktrees.some((item) => item.id === 'desktop-e2e')).toBe(true)
    await desktop.page.evaluate(
      (id) => window.loopalDesktop.removeWorktree({
        workspaceId: id,
        name: 'desktop-e2e',
        force: false,
      }),
      workspaceId,
    )

    await assertRootConfinement(desktop.page, workspaceId)
    await assertWatch(desktop.page, desktop.project)
  } finally {
    await closeDesktop(desktop)
  }
})

async function readMain(page: import('@playwright/test').Page, workspaceId: string) {
  return page.evaluate(
    (id) => window.loopalDesktop.readFile({ workspaceId: id, path: 'src/main.rs' }),
    workspaceId,
  )
}

async function assertRootConfinement(
  page: import('@playwright/test').Page,
  workspaceId: string,
): Promise<void> {
  await expect(page.evaluate(
    (id) => window.loopalDesktop.readFile({ workspaceId: id, path: '../outside.txt' }),
    workspaceId,
  )).rejects.toThrow(/workspace|path/i)
  if (process.platform !== 'win32') {
    await expect(page.evaluate(
      (id) => window.loopalDesktop.readFile({ workspaceId: id, path: 'escape-link' }),
      workspaceId,
    )).rejects.toThrow(/workspace|root|escape/i)
  }
}

async function assertWatch(page: import('@playwright/test').Page, project: string): Promise<void> {
  await page.evaluate(() => {
    const state = window as typeof window & { workspaceEvents?: string[] }
    state.workspaceEvents = []
    window.loopalDesktop.onEvent((event) => {
      if (event.type === 'file_changed') state.workspaceEvents?.push(event.path)
    })
  })
  await writeFile(join(project, 'watch-marker.txt'), 'watch me\n')
  await expect.poll(() => page.evaluate(() => {
    const state = window as typeof window & { workspaceEvents?: string[] }
    return state.workspaceEvents?.includes('watch-marker.txt') ?? false
  })).toBe(true)
}
