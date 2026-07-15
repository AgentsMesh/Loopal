import { createSessionDirectoryCommand, type SessionDirectoryCommandRunner } from './session-directory-command'

describe('Session directory one-shot command bridge', () => {
  it('passes bounded no-shell arguments and a secret-minimized environment', async () => {
    const runner = vi.fn<SessionDirectoryCommandRunner>(async () => ({
      stdout: JSON.stringify({ ok: true, value: { path: '/project', name: 'project' } }),
    }))
    const allowedGit = {
      GIT_CONFIG_NOSYSTEM: '1', GIT_CONFIG_GLOBAL: '/tmp/gitconfig',
      GIT_AUTHOR_NAME: 'Loopal Author', GIT_AUTHOR_EMAIL: 'author@loopal.invalid',
      GIT_AUTHOR_DATE: '2000-01-01T00:00:00Z', GIT_COMMITTER_NAME: 'Loopal Committer',
      GIT_COMMITTER_EMAIL: 'committer@loopal.invalid',
      GIT_COMMITTER_DATE: '2000-01-02T00:00:00Z',
    }
    vi.stubEnv('ANTHROPIC_API_KEY', 'secret')
    for (const [key, value] of Object.entries(allowedGit)) vi.stubEnv(key, value)
    for (const key of [
      'GIT_DIR', 'GIT_WORK_TREE', 'GIT_CONFIG_COUNT', 'GIT_CONFIG_KEY_0', 'GIT_CONFIG_VALUE_0',
    ]) vi.stubEnv(key, 'injected')
    const request = createSessionDirectoryCommand('/bin/loopal', runner)
    await expect(request('desktop/inspectWorkingDirectory', { path: '/project' }))
      .resolves.toEqual({ path: '/project', name: 'project' })
    expect(runner).toHaveBeenCalledWith(
      '/bin/loopal', ['desktop', 'inspect-directory', '--path', '/project'],
      expect.objectContaining({ timeout: 30_000, maxBuffer: 1024 * 1024 }),
    )
    const options = runner.mock.calls[0]![2]
    expect(options.env.ANTHROPIC_API_KEY).toBeUndefined()
    expect(options.env).toMatchObject(allowedGit)
    expect(options.env).not.toHaveProperty('GIT_DIR')
    expect(options.env).not.toHaveProperty('GIT_WORK_TREE')
    expect(options.env).not.toHaveProperty('GIT_CONFIG_COUNT')
    expect(options.env).not.toHaveProperty('GIT_CONFIG_KEY_0')
    expect(options.env).not.toHaveProperty('GIT_CONFIG_VALUE_0')
    expect(options.env.LOOPAL_OTEL_ENABLED).toBe('false')
    vi.unstubAllEnvs()
  })

  it('rejects malformed and domain-error envelopes', async () => {
    const malformed = createSessionDirectoryCommand('/bin/loopal', async () => ({
      stdout: '{"ok":true,"value":{},"unexpected":true}',
    }))
    await expect(malformed('desktop/inspectWorkingDirectory', { path: '/project' }))
      .rejects.toThrow()
    const rejected = createSessionDirectoryCommand('/bin/loopal', async () => ({
      stdout: JSON.stringify({
        ok: false, error: { code: 'not_git_repository', message: 'Git required' },
      }),
    }))
    await expect(rejected('desktop/prepareWorktree', {
      path: '/project', name: 'wt', expectedRoot: '/project', expectedHead: 'abc123',
    }))
      .rejects.toThrow('not_git_repository: Git required')
  })
})
