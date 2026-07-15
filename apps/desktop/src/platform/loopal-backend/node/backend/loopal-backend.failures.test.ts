import { describe, expect, it, vi } from 'vitest'
import { CancellationToken } from '../../../../base/common/cancellation'
import { type SessionSummary } from '../../../../shared/contracts'
import { createBackend } from './loopal-backend.test-fixtures'

describe('LoopalDesktopBackend failure and concurrency boundaries', () => {
  it('uses default clock/workspace name and bootstraps on the first operation', async () => {
    const { backend } = createBackend({ cwd: '/', defaultClock: true })
    await backend.sendMessage('session-1', 'bootstrap implicitly')
    const snapshot = await backend.bootstrap()
    expect(snapshot.workspaces[0]!.name).toBe('Workspace')
    expect(Date.parse(snapshot.sessions[0]!.updatedAt)).not.toBeNaN()
  })

  it('keeps concurrent open read-only and validates missing sessions and workspaces', async () => {
    const { backend, hosts } = createBackend()
    await backend.bootstrap()
    const [first, second] = await Promise.all([
      backend.openSession('session-2'), backend.openSession('session-2'),
    ])
    expect(first.session.id).toBe('session-2')
    expect(second.session.id).toBe('session-2')
    expect(hosts).toHaveLength(1)
    await expect(backend.stopSession('missing')).rejects.toThrow('Unknown')
    await expect(backend.restartSession('missing')).rejects.toThrow('Unknown')
    await expect(backend.createSession({
      authorizationId: '5d0c638c-d44c-4f47-818b-62e6b599e31c', launchMode: 'directory',
    })).rejects.toThrow('authorization')
    await expect(backend.gitStatus('other', CancellationToken.None)).rejects.toThrow('Unknown workspace')
  })

  it('treats stopping an already stopped catalog session as idempotent', async () => {
    const { backend, hosts } = createBackend()
    await backend.bootstrap()
    await backend.stopSession('session-2')
    expect(hosts).toHaveLength(1)
    await backend.stopSession('session-1')
    await expect(backend.stopSession('session-1')).resolves.toBeUndefined()
    expect(hosts[0]!.stop).toHaveBeenCalledOnce()
  })

  it('rejects restarting archived sessions and sending into stopped sessions', async () => {
    const { backend } = createBackend()
    const snapshot = await backend.bootstrap()
    await backend.stopSession('session-1')
    await expect(backend.sendMessage('session-1', 'must not resume'))
      .rejects.toThrow('restart it first')
    const internals = backend as unknown as {
      directory: { sessions: Map<string, SessionSummary> }
    }
    const session = snapshot.sessions.find(({ id }) => id === 'session-2')!
    internals.directory.sessions.set(session.id, { ...session, status: 'archived' })
    await expect(backend.restartSession(session.id)).rejects.toThrow('Archived session')
  })

  it('retires a newly created runtime when catalog initialization fails', async () => {
    const { backend, hosts } = createBackend({
      sessionDirectoryRequest: async () => ({ path: '/workspace/project', name: 'project' }),
      hostSetup: (host, index) => {
        if (index !== 1) return
        host.request.mockRejectedValue(new Error('catalog failed'))
        host.stop.mockRejectedValue(new Error('cleanup failed'))
      },
    })
    await backend.bootstrap()
    const selected = await backend.authorizeSessionDirectory('/workspace/project')
    await expect(backend.createSession({
      authorizationId: selected.authorizationId, launchMode: 'directory',
    }))
      .rejects.toThrow('catalog failed')
    await vi.waitFor(() => expect(hosts[1]!.dispose).toHaveBeenCalledOnce())
  })

  it('restores a direct directory grant when Host startup fails', async () => {
    const { backend, hosts } = createBackend({
      sessionDirectoryRequest: async () => ({ path: '/project', name: 'project' }),
      hostSetup: (host, index) => {
        if (index === 1) host.start.mockRejectedValueOnce(new Error('Host failed'))
      },
    })
    await backend.bootstrap()
    const selected = await backend.authorizeSessionDirectory('/project')
    const input = { authorizationId: selected.authorizationId, launchMode: 'directory' as const }
    await expect(backend.createSession(input)).rejects.toThrow('Host failed')
    await expect(backend.createSession(input)).resolves.toMatchObject({
      session: { status: 'waiting' },
    })
    expect(hosts).toHaveLength(3)
  })

  it('rolls back a clean prepared worktree and retains an unsafe one explicitly', async () => {
    let cleanupFails = false
    const request = vi.fn(async (method: string) => {
      if (method.endsWith('inspectWorkingDirectory')) return {
        path: '/project', name: 'project',
        git: { root: '/project', head: 'a'.repeat(40), dirty: false },
      }
      if (method.endsWith('prepareWorktree')) return {
        path: '/project/.loopal/worktrees/wt', branch: 'loopal-wt-wt', name: 'wt',
      }
      if (cleanupFails) throw new Error('worktree_cleanup_unsafe: dirty worktree')
      return { path: '/project/.loopal/worktrees/wt', removed: true }
    })
    const { backend } = createBackend({
      sessionDirectoryRequest: request,
      hostSetup: (host, index) => {
        if (index > 0) host.start.mockRejectedValueOnce(new Error('Host failed'))
      },
    })
    await backend.bootstrap()
    const clean = await backend.authorizeSessionDirectory('/project')
    await expect(backend.createSession({
      authorizationId: clean.authorizationId, launchMode: 'worktree', worktreeName: 'wt',
    })).rejects.toThrow('Host failed')
    expect((await backend.bootstrap()).workspaces.some(
      ({ rootUri }) => rootUri.includes('/.loopal/worktrees/wt'),
    )).toBe(false)
    await expect(backend.createSession({
      authorizationId: clean.authorizationId, launchMode: 'directory',
    })).rejects.toThrow('Host failed')

    cleanupFails = true
    const unsafe = await backend.authorizeSessionDirectory('/project')
    await expect(backend.createSession({
      authorizationId: unsafe.authorizationId, launchMode: 'worktree', worktreeName: 'wt',
    })).rejects.toThrow(
      'worktree_retained: session creation failed (Host failed); cleanup failed and the worktree '
        + 'was retained at /project/.loopal/worktrees/wt',
    )
    await expect(backend.createSession({
      authorizationId: unsafe.authorizationId, launchMode: 'directory',
    })).rejects.toThrow('directory_authorization_invalid')
  })

  it('retains the cwd and consumes its grant after Loopal created the session', async () => {
    const methods: string[] = []
    const request = vi.fn(async (method: string) => {
      methods.push(method)
      if (method.endsWith('inspectWorkingDirectory')) return {
        path: '/project', name: 'project',
        git: { root: '/project', head: 'a'.repeat(40), dirty: false },
      }
      if (method.endsWith('prepareWorktree')) return {
        path: '/project/.loopal/worktrees/recover',
        branch: 'loopal-wt-recover', name: 'recover',
      }
      return { path: '/project/.loopal/worktrees/recover', removed: true }
    })
    const { backend } = createBackend({
      sessionDirectoryRequest: request,
      hostSetup: (host, index) => {
        if (index === 1) host.request.mockRejectedValue(new Error('snapshot failed'))
      },
    })
    await backend.bootstrap()
    const selected = await backend.authorizeSessionDirectory('/project')
    await expect(backend.createSession({
      authorizationId: selected.authorizationId,
      launchMode: 'worktree', worktreeName: 'recover',
    })).rejects.toThrow(
      'session_created_recovery_required: Loopal session session-3 was created at '
        + '/project/.loopal/worktrees/recover; Desktop initialization failed (snapshot failed)',
    )
    expect(methods).not.toContain('desktop/cleanupWorktree')
    expect((await backend.bootstrap()).workspaces.some(
      ({ rootUri }) => rootUri.includes('/.loopal/worktrees/recover'),
    )).toBe(true)
    await expect(backend.createSession({
      authorizationId: selected.authorizationId, launchMode: 'directory',
    })).rejects.toThrow('directory_authorization_invalid')
  })

  it('does not implicitly resume a Host for workspace operations', async () => {
    const { backend } = createBackend()
    await backend.bootstrap()
    await backend.stopSession('session-1')
    await expect(backend.gitStatus('local-workspace', CancellationToken.None))
      .rejects.toThrow('no live runtime')
  })
})
