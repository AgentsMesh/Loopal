import { expect, type Page } from '@playwright/test'
import { realpath } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'
import { type DesktopFixture } from '../electron/electron-fixture'
import { activeDetail, ready, runtimeTarget } from '../runtime/llm-e2e-helpers'

export async function createFromDirectory(
  page: Page,
  mode: 'directory' | 'worktree',
  git = false,
  name?: string,
): Promise<void> {
  await openWithSelectedDirectory(page)
  const dialog = page.getByTestId('new-session-dialog')
  await expect(dialog.getByTestId('launch-direct')).toBeChecked()
  if (git) await expect(dialog.getByTestId('launch-worktree')).toBeVisible()
  else await expect(dialog.getByTestId('launch-worktree')).toHaveCount(0)
  if (mode === 'worktree') {
    await dialog.getByTestId('launch-worktree').check()
    await expect(dialog).toContainText(/uncommitted changes are not copied/i)
    await dialog.getByTestId('worktree-name').fill(name!)
  }
  await dialog.getByTestId('create-session-confirm').click()
  await expect(dialog).toHaveCount(0, { timeout: 30_000 })
  await ready(page)
}

export async function openWithSelectedDirectory(page: Page): Promise<void> {
  await page.locator('.new-session').click()
  const dialog = page.getByTestId('new-session-dialog')
  await expect(dialog).toBeVisible()
  await dialog.getByTestId('session-directory').click()
  await expect(dialog.getByTestId('create-session-confirm')).toBeEnabled()
}

export async function expectActiveDirectory(
  page: Page,
  expected: string,
  kind: 'folder' | 'git_worktree',
): Promise<void> {
  const state = await page.evaluate(async () => {
    const bootstrap = await window.loopalDesktop.bootstrap()
    const session = bootstrap.sessions.find((item) => item.id === bootstrap.activeSessionId)!
    return {
      session,
      workspace: bootstrap.workspaces.find((item) => item.id === session.workspaceId)!,
    }
  })
  const actual = state.workspace.rootUri.startsWith('file:')
    ? fileURLToPath(state.workspace.rootUri) : state.workspace.rootUri
  expect(await realpath(actual)).toBe(await realpath(expected))
  expect(state.workspace.kind).toBe(kind)
  expect(state.session.workspaceId).toBe(state.workspace.id)
}

export async function enableTools(page: Page): Promise<void> {
  const target = await runtimeTarget(page)
  await page.evaluate((value) => window.loopalDesktop.controlAgent({
    target: value, command: { type: 'permission', mode: 'bypass' },
  }), target)
}

export async function stopActive(page: Page): Promise<void> {
  const detail = await activeDetail(page)
  await page.evaluate((id) => window.loopalDesktop.stopSession(id), detail.session.id)
}

export async function sessionCount(page: Page): Promise<number> {
  return page.evaluate(async () => (await window.loopalDesktop.bootstrap()).sessions.length)
}

export async function stopAllLiveSessions(page: Page): Promise<void> {
  const ids = await page.evaluate(async () => (await window.loopalDesktop.bootstrap()).runtimes
    .filter((runtime) => !['stopped', 'crashed'].includes(runtime.state))
    .map((runtime) => runtime.sessionId))
  for (const id of ids) await page.evaluate((sessionId) => (
    window.loopalDesktop.stopSession(sessionId)
  ), id)
  await expect.poll(() => page.evaluate(async () => (await window.loopalDesktop.bootstrap())
    .runtimes.filter((runtime) => !['stopped', 'crashed'].includes(runtime.state)).length)).toBe(0)
}

export async function expectHidden(desktop: DesktopFixture): Promise<void> {
  await expect.poll(() => desktop.app.evaluate(({ BrowserWindow }) => {
    const current = BrowserWindow.getAllWindows()[0]
    return { visible: current?.isVisible(), focused: current?.isFocused() }
  })).toEqual({ visible: false, focused: false })
}
