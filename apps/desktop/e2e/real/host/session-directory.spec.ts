import { expect, test } from '@playwright/test'
import { access, rm, writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import {
  closeDesktop, launchDesktop, relaunchDesktop,
} from '../../support/electron/electron-fixture'
import { activeDetail, ready, send } from '../../support/runtime/llm-e2e-helpers'
import {
  createSessionDirectory, gitOutput, queueSessionDirectories,
} from '../../support/fixtures/session-directory-fixture'
import {
  createFromDirectory, enableTools, expectActiveDirectory, expectHidden,
  openWithSelectedDirectory, sessionCount, stopActive, stopAllLiveSessions,
} from '../../support/fixtures/session-directory-ui'
test('creates directory and isolated worktree Sessions without focusing Electron', async () => {
  let desktop = await launchDesktop('real', 'session-directories')
  try {
    const page = desktop.page
    const current = page.getByTestId('current-session-list')
    await expect(current).toBeVisible()
    await expect(page.getByTestId('history-session-list')).toHaveCount(0)
    await expect(page.getByLabel('Active workspace')).toHaveCount(0)
    await expect(page.getByLabel('Active session')).toHaveCount(0)
    const plain = await createSessionDirectory(desktop, 'plain workspace', false)
    const git = await createSessionDirectory(desktop, 'git workspace', true)
    const worktreeGit = await createSessionDirectory(desktop, 'worktree workspace', true)
    const gitSubdirectory = join(worktreeGit.path, 'src')
    expect(await gitOutput(desktop, worktreeGit.path,
      ['ls-files', '--error-unmatch', 'src/main.rs'])).toBe('src/main.rs')
    await queueSessionDirectories(desktop,
      [null, plain.path, git.path, gitSubdirectory, worktreeGit.path])
    const initialSessions = await sessionCount(desktop.page)

    await desktop.page.locator('.new-session').click()
    const firstDialog = desktop.page.getByTestId('new-session-dialog')
    await expect(firstDialog).toBeVisible()
    await expect(firstDialog).toContainText(/New session/i)
    await firstDialog.getByTestId('session-directory').click()
    await expect(firstDialog.getByTestId('create-session-confirm')).toBeDisabled()
    await expectHidden(desktop)
    expect(await sessionCount(desktop.page)).toBe(initialSessions)
    await firstDialog.getByTestId('create-session-cancel').click()
    await expect(firstDialog).toHaveCount(0)
    expect(await sessionCount(desktop.page)).toBe(initialSessions)

    await createFromDirectory(desktop.page, 'directory')
    await expectActiveDirectory(desktop.page, plain.path, 'folder')
    await enableTools(desktop.page)
    await send(desktop.page, 'Verify the plain directory')
    await expect(desktop.page.getByTestId('conversation')).toContainText(
      plain.path, { timeout: 20_000 },
    )
    await expect(desktop.page.getByTestId('conversation'))
      .toContainText('Plain directory verified.', { timeout: 20_000 })
    const plainSession = (await activeDetail(page)).session
    await expect(current.locator(`[data-session-id="${plainSession.id}"]`)).toBeVisible()
    const visibleIds = await current.locator('.session-card').evaluateAll((cards) => (
      cards.map((card) => card.getAttribute('data-session-id'))
    ))
    const visibleWorkspaceCount = await page.evaluate(async (ids) => new Set(
      (await window.loopalDesktop.bootstrap()).sessions
        .filter(({ id }) => ids.includes(id)).map(({ workspaceId }) => workspaceId),
    ).size, visibleIds)
    expect(visibleWorkspaceCount).toBeGreaterThanOrEqual(2)
    await stopActive(desktop.page)
    await expect(current.locator(`[data-session-id="${plainSession.id}"]`)).toHaveCount(0)
    await expect(page.getByTestId('history-session-list')).toHaveCount(0)
    const search = page.getByLabel('Search sessions')
    await search.fill(plainSession.title)
    const history = page.getByTestId('history-session-list')
    await expect(history.locator(`[data-session-id="${plainSession.id}"]`)).toBeVisible()
    await history.locator(`[data-session-id="${plainSession.id}"]`).click()
    await expect(page.getByTestId('active-session-title')).toHaveText(plainSession.title)
    await search.fill('')
    await createFromDirectory(desktop.page, 'directory', true)
    await expectActiveDirectory(desktop.page, git.path, 'folder')
    expect(await gitOutput(desktop, git.path, ['branch', '--show-current'])).toBe('main')
    expect(((await gitOutput(desktop, git.path, ['worktree', 'list', '--porcelain']))
      .match(/^worktree /gm) ?? [])).toHaveLength(1)
    await enableTools(desktop.page)
    await send(desktop.page, 'Verify the Git directory directly')
    await expect(desktop.page.getByTestId('conversation')).toContainText(
      'Git directory verified.', { timeout: 20_000 },
    )
    await stopAllLiveSessions(desktop.page)
    expect(await gitOutput(desktop, worktreeGit.path, ['status', '--porcelain'])).toBe('')
    await createFromDirectory(desktop.page, 'worktree', true, 'e2e-isolated')
    const worktreeRoot = join(worktreeGit.path, '.loopal', 'worktrees', 'e2e-isolated')
    const worktreeDirectory = join(worktreeRoot, 'src')
    await expectActiveDirectory(desktop.page, worktreeDirectory, 'git_worktree')
    expect(await gitOutput(desktop, worktreeDirectory, ['branch', '--show-current']))
      .toBe('loopal-wt-e2e-isolated')
    expect(await gitOutput(desktop, worktreeGit.path, ['status', '--porcelain'])).toBe('')
    await enableTools(desktop.page)
    await send(desktop.page, 'Verify the isolated worktree')
    await expect(desktop.page.getByTestId('conversation')).toContainText(
      'Worktree isolation verified.', { timeout: 20_000 },
    )
    await expect(access(join(worktreeDirectory, 'worktree-only.txt'))).resolves.toBeUndefined()
    await expect(access(join(gitSubdirectory, 'worktree-only.txt'))).rejects.toThrow()

    const worktreeSession = (await activeDetail(desktop.page)).session.id
    await desktop.page.evaluate((id) => window.loopalDesktop.stopSession(id), worktreeSession)
    await desktop.page.evaluate((id) => window.loopalDesktop.restartSession(id), worktreeSession)
    await ready(desktop.page)
    await expectActiveDirectory(desktop.page, worktreeDirectory, 'git_worktree')
    await enableTools(desktop.page)
    await send(desktop.page, 'Verify the restarted isolated worktree')
    await expect(desktop.page.getByTestId('conversation')).toContainText(
      'Restarted worktree verified.', { timeout: 20_000 },
    )
    await ready(desktop.page)

    desktop = await relaunchDesktop(desktop)
    await ready(desktop.page)
    await expectActiveDirectory(desktop.page, worktreeDirectory, 'git_worktree')
    await enableTools(desktop.page)
    await send(desktop.page, 'Verify the relaunched isolated worktree')
    await expect(desktop.page.getByTestId('conversation')).toContainText(
      'Relaunched worktree verified.', { timeout: 20_000 },
    )
    await expect.poll(() => desktop.llm!.state()).toMatchObject({
      name: 'session-directories', served: 10, remaining: 0, verified: true,
    })

    await queueSessionDirectories(desktop, [worktreeGit.path])
    const beforeFailure = await sessionCount(desktop.page)
    const dirtyFile = join(worktreeGit.path, 'dirty-e2e.txt')
    await writeFile(dirtyFile, 'not committed\n')
    await openWithSelectedDirectory(desktop.page)
    const duplicate = desktop.page.getByTestId('new-session-dialog')
    await duplicate.getByTestId('launch-worktree').check()
    await expect(duplicate).toContainText(/uncommitted changes are not copied/i)
    await duplicate.getByTestId('worktree-name').fill('../escape')
    await expect(duplicate.getByTestId('create-session-confirm')).toBeDisabled()
    await duplicate.getByTestId('worktree-name').fill('e2e-isolated')
    await duplicate.getByTestId('create-session-confirm').click()
    await expect(duplicate.getByTestId('session-create-error'))
      .toContainText(/exists|already|已存在/i)
    expect(await sessionCount(desktop.page)).toBe(beforeFailure)
    await duplicate.getByTestId('create-session-cancel').click()
    await rm(dirtyFile)
    expect(await gitOutput(desktop, worktreeGit.path, ['status', '--porcelain'])).toBe('')

    await desktop.page.getByRole('button', { name: 'Settings' }).click()
    await desktop.page.getByTestId('desktop-language').selectOption('zh-CN')
    await desktop.page.getByRole('button', { name: '关闭设置' }).click()
    await expect(desktop.page.locator('html')).toHaveAttribute('lang', 'zh-CN')
    await expect(desktop.page.getByLabel('搜索会话')).toBeVisible()
    await expect(desktop.page.getByTestId('current-session-list')).toContainText('当前会话')
    await expect(desktop.page.getByTestId('history-session-list')).toHaveCount(0)
    await desktop.page.locator('.new-session').click()
    const chinese = desktop.page.getByTestId('new-session-dialog')
    await expect(chinese).toContainText('新建会话')
    await expect(chinese).toContainText('运行目录')
    await expect(chinese.getByTestId('session-directory')).toContainText('选择目录…')
    await expect(chinese.getByTestId('create-session-confirm')).toContainText('创建会话')
    await expect(chinese.getByTestId('create-session-cancel')).toContainText('取消')
    await chinese.getByTestId('create-session-cancel').click()
    await expectHidden(desktop)
  } finally {
    await closeDesktop(desktop)
  }
})
