import { execFile as execFileCallback } from 'node:child_process'
import { mkdtemp, mkdir, realpath, rm, stat, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { promisify } from 'node:util'
import { FakeSessionDirectoryAuthority } from './fake-session-directory'

const execFile = promisify(execFileCallback)

describe('FakeSessionDirectoryAuthority', () => {
  it('preserves a selected Git subdirectory inside its worktree', async () => {
    const root = await mkdtemp(join(tmpdir(), 'loopal-fake-directory-'))
    const nested = join(root, 'nested')
    try {
      await mkdir(nested)
      await writeFile(join(nested, 'tracked.txt'), 'fixture\n')
      await git(root, ['init', '-q', '--initial-branch=main'])
      await git(root, ['config', 'user.name', 'Loopal Test'])
      await git(root, ['config', 'user.email', 'loopal@example.invalid'])
      await git(root, ['add', '.'])
      await git(root, ['commit', '-qm', 'fixture'])
      const authority = new FakeSessionDirectoryAuthority()
      const selected = await authority.authorize(nested)
      const prepared = await authority.prepare({
        authorizationId: selected.authorizationId,
        launchMode: 'worktree', worktreeName: 'nested-test',
      })
      const expected = join(root, '.loopal', 'worktrees', 'nested-test', 'nested')
      expect(await realpath(prepared.path)).toBe(await realpath(expected))
      expect((await stat(join(prepared.path, 'tracked.txt'))).isFile()).toBe(true)
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })
})

async function git(cwd: string, args: readonly string[]): Promise<void> {
  await execFile('git', args, { cwd })
}
