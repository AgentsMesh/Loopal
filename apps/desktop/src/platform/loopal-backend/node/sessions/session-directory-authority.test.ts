import { SessionDirectoryAuthority } from './session-directory-authority'

describe('SessionDirectoryAuthority', () => {
  it('consumes an opaque authorization exactly once', async () => {
    const request = vi.fn(async (method: string) => method.endsWith('inspectWorkingDirectory')
      ? { path: '/project', name: 'project' }
      : { path: '/project/.loopal/worktrees/wt', branch: 'loopal-wt-wt', name: 'wt' })
    const authority = new SessionDirectoryAuthority(request)
    const selected = await authority.authorize('/project')
    await expect(authority.prepare({
      authorizationId: selected.authorizationId, launchMode: 'directory',
    })).resolves.toEqual({ path: '/project', kind: 'folder' })
    await expect(authority.prepare({
      authorizationId: selected.authorizationId, launchMode: 'directory',
    })).rejects.toThrow('directory_authorization_invalid')
  })

  it('expires bounded grants and refuses worktrees outside Git', async () => {
    let now = 10
    const authority = new SessionDirectoryAuthority(
      async () => ({ path: '/project', name: 'project' }), () => now, 5,
    )
    const selected = await authority.authorize('/project')
    now = 15
    await expect(authority.prepare({
      authorizationId: selected.authorizationId, launchMode: 'directory',
    })).rejects.toThrow('directory_authorization_invalid')

    now = 20
    const next = await authority.authorize('/project')
    await expect(authority.prepare({
      authorizationId: next.authorizationId, launchMode: 'worktree', worktreeName: 'wt',
    })).rejects.toThrow('not_git_repository')
  })

  it('rejects extra fields from the Rust inspection boundary', async () => {
    const authority = new SessionDirectoryAuthority(async () => ({
      path: '/project', name: 'project', injected: true,
    }))
    await expect(authority.authorize('/project')).rejects.toThrow()
  })

  it('revalidates direct directories and restores the grant after rollback', async () => {
    let root = '/repo'
    const authority = new SessionDirectoryAuthority(async () => ({
      path: '/repo/project', name: 'project',
      git: { root, branch: 'main', head: 'a'.repeat(40), dirty: false },
    }))
    const selected = await authority.authorize('/repo/project')
    root = '/replacement'
    await expect(authority.claim({
      authorizationId: selected.authorizationId, launchMode: 'directory',
    })).rejects.toThrow('working_directory_changed')
    root = '/repo'
    const claim = await authority.claim({
      authorizationId: selected.authorizationId, launchMode: 'directory',
    })
    await claim.rollback()
    await expect(authority.prepare({
      authorizationId: selected.authorizationId, launchMode: 'directory',
    })).resolves.toMatchObject({ path: '/repo/project', kind: 'folder' })
  })

  it('keeps the grant after a recoverable worktree error', async () => {
    let prepareAttempts = 0
    const authority = new SessionDirectoryAuthority(async (method) => {
      if (method.endsWith('inspectWorkingDirectory')) return {
        path: '/project', name: 'project',
        git: { root: '/project', branch: 'main', dirty: false },
      }
      prepareAttempts += 1
      if (prepareAttempts === 1) throw new Error('worktree_exists: choose another name')
      return { path: '/project/.loopal/worktrees/next', branch: 'loopal-wt-next', name: 'next' }
    })
    const selected = await authority.authorize('/project')
    await expect(authority.prepare({
      authorizationId: selected.authorizationId, launchMode: 'worktree', worktreeName: 'used',
    })).rejects.toThrow('worktree_exists')
    await expect(authority.prepare({
      authorizationId: selected.authorizationId, launchMode: 'worktree', worktreeName: 'next',
    })).resolves.toMatchObject({ branch: 'loopal-wt-next' })
  })

  it('consumes the grant when Rust retains an unsafe prepared worktree', async () => {
    const authority = new SessionDirectoryAuthority(async (method) => {
      if (method.endsWith('inspectWorkingDirectory')) return {
        path: '/project', name: 'project',
        git: { root: '/project', branch: 'main', head: 'a'.repeat(40), dirty: false },
      }
      throw new Error(
        'worktree_retained: worktree retained at /project/.loopal/worktrees/wt',
      )
    })
    const selected = await authority.authorize('/project')

    await expect(authority.prepare({
      authorizationId: selected.authorizationId, launchMode: 'worktree', worktreeName: 'wt',
    })).rejects.toThrow('worktree_retained')
    await expect(authority.prepare({
      authorizationId: selected.authorizationId, launchMode: 'directory',
    })).rejects.toThrow('directory_authorization_invalid')
  })

  it('claims worktree authorization before awaiting and restores it only after failure', async () => {
    let release!: () => void
    const gate = new Promise<void>((resolve) => { release = resolve })
    let attempts = 0
    const authority = new SessionDirectoryAuthority(async (method) => {
      if (method.endsWith('inspectWorkingDirectory')) return {
        path: '/project', name: 'project',
        git: { root: '/project', branch: 'main', dirty: false },
      }
      attempts += 1
      if (attempts === 1) {
        await gate
        throw new Error('worktree_exists: retry')
      }
      return { path: '/project/.loopal/worktrees/retry', branch: 'loopal-wt-retry', name: 'retry' }
    })
    const selected = await authority.authorize('/project')
    const first = authority.prepare({
      authorizationId: selected.authorizationId, launchMode: 'worktree', worktreeName: 'first',
    })
    await vi.waitFor(() => expect(attempts).toBe(1))
    await expect(authority.prepare({
      authorizationId: selected.authorizationId, launchMode: 'worktree', worktreeName: 'second',
    })).rejects.toThrow('directory_authorization_invalid')
    expect(attempts).toBe(1)
    const failed = expect(first).rejects.toThrow('worktree_exists')
    release()
    await failed
    await expect(authority.prepare({
      authorizationId: selected.authorizationId, launchMode: 'worktree', worktreeName: 'retry',
    })).resolves.toMatchObject({ branch: 'loopal-wt-retry' })
  })

  it('cleans a prepared worktree before restoring a rolled-back grant', async () => {
    const calls: string[] = []
    const params: unknown[] = []
    const authority = new SessionDirectoryAuthority(async (method, input) => {
      calls.push(method)
      params.push(input)
      if (method.endsWith('inspectWorkingDirectory')) return {
        path: '/project', name: 'project',
        git: { root: '/project', head: 'a'.repeat(40), dirty: false },
      }
      if (method.endsWith('prepareWorktree')) return {
        path: '/project/.loopal/worktrees/wt', branch: 'loopal-wt-wt', name: 'wt',
      }
      return { path: '/project/.loopal/worktrees/wt', removed: true }
    })
    const selected = await authority.authorize('/project')
    expect(selected.git).toEqual({ root: '/project', dirty: false })
    expect(selected.git).not.toHaveProperty('head')
    const claim = await authority.claim({
      authorizationId: selected.authorizationId, launchMode: 'worktree', worktreeName: 'wt',
    })
    expect(params[1]).toMatchObject({ expectedHead: 'a'.repeat(40) })
    await claim.rollback()
    expect(calls).toContain('desktop/cleanupWorktree')
    await expect(authority.prepare({
      authorizationId: selected.authorizationId, launchMode: 'directory',
    })).resolves.toMatchObject({ path: '/project' })
  })
})
