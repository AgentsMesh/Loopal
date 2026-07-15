import { mkdtemp, readFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { type Workspace } from '../../../../shared/contracts'
import { createBackend } from '../backend/loopal-backend.test-fixtures'

describe('Loopal session creation activation', () => {
  let root = ''

  afterEach(async () => {
    if (root) await rm(root, { recursive: true, force: true })
    root = ''
  })

  it('retains and persists a worktree when startup fails after session creation', async () => {
    root = await mkdtemp(join(tmpdir(), 'loopal-created-session-'))
    const statePath = join(root, 'sessions.json')
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
    const { backend, hosts } = createBackend({
      sessionStatePath: statePath,
      sessionDirectoryRequest: request,
      hostSetup: (host, index) => {
        if (index !== 1) return
        host.emitSessionCreated = true
        host.failAfterSessionCreated = new Error('root_view_not_ready')
      },
    })
    await backend.bootstrap()
    const selected = await backend.authorizeSessionDirectory('/project')
    const input = {
      authorizationId: selected.authorizationId,
      launchMode: 'worktree' as const,
      worktreeName: 'recover',
    }

    await expect(backend.createSession(input)).rejects.toThrow(
      'session_created_recovery_required: Loopal session session-3 was created',
    )
    expect(hosts[1]!.start).toHaveBeenCalledWith(expect.any(Function))
    expect(methods).not.toContain('desktop/cleanupWorktree')
    expect((await backend.bootstrap()).workspaces).toContainEqual(expect.objectContaining({
      rootUri: 'file:///project/.loopal/worktrees/recover', kind: 'git_worktree',
    }))
    const persisted = JSON.parse(await readFile(statePath, 'utf8'))
    expect(persisted.runningSessionIds).not.toContain('session-3')
    expect(persisted.sessionLocations).toContainEqual(expect.objectContaining({
      sessionId: 'session-3', cwd: '/project/.loopal/worktrees/recover',
    }))
    await expect(backend.createSession({
      authorizationId: input.authorizationId, launchMode: 'directory',
    }))
      .rejects.toThrow('directory_authorization_invalid')
  })

  it('commits the workspace catalog before buffered runtime events flush', async () => {
    const { backend } = createBackend({
      sessionDirectoryRequest: async () => ({ path: '/project/feature', name: 'feature' }),
      hostSetup: (host, index) => { if (index === 1) host.emitSessionCreated = true },
    })
    await backend.bootstrap()
    const visibility: boolean[] = []
    const internals = backend as unknown as {
      workspaces: { values(): readonly Workspace[] }
    }
    backend.onEvent((event) => {
      if (event.type !== 'runtime_updated' || event.runtime.sessionId !== 'session-3') return
      visibility.push(internals.workspaces.values().some(
        ({ rootUri }) => rootUri === 'file:///project/feature',
      ))
    })
    const selected = await backend.authorizeSessionDirectory('/project/feature')

    await backend.createSession({
      authorizationId: selected.authorizationId, launchMode: 'directory',
    })
    expect(visibility.length).toBeGreaterThan(0)
    expect(visibility.every(Boolean)).toBe(true)
  })

  it.each([
    'desktop_protocol_drain_incomplete: stdout did not close after SIGKILL',
    'desktop_process_termination_unconfirmed: Host did not exit after SIGKILL',
    'desktop_session_creation_state_unknown: Host failed after ALIVE',
  ])('retains an authorized worktree when creation is unknown: %s', async (failure) => {
    const methods: string[] = []
    const request = vi.fn(async (method: string) => {
      methods.push(method)
      if (method.endsWith('inspectWorkingDirectory')) return {
        path: '/project', name: 'project',
        git: { root: '/project', head: 'b'.repeat(40), dirty: false },
      }
      if (method.endsWith('prepareWorktree')) return {
        path: '/project/.loopal/worktrees/unknown',
        branch: 'loopal-wt-unknown', name: 'unknown',
      }
      return { path: '/project/.loopal/worktrees/unknown', removed: true }
    })
    const { backend } = createBackend({
      sessionDirectoryRequest: request,
      hostSetup: (host, index) => {
        if (index === 1) host.start.mockRejectedValueOnce(new Error(failure))
      },
    })
    await backend.bootstrap()
    const selected = await backend.authorizeSessionDirectory('/project')
    const input = {
      authorizationId: selected.authorizationId,
      launchMode: 'worktree' as const,
      worktreeName: 'unknown',
    }

    await expect(backend.createSession(input)).rejects.toThrow(
      'worktree_retained: session_creation_state_unknown',
    )
    expect(methods).not.toContain('desktop/cleanupWorktree')
    expect((await backend.bootstrap()).workspaces).toContainEqual(expect.objectContaining({
      rootUri: 'file:///project/.loopal/worktrees/unknown',
    }))
    await expect(backend.createSession({
      authorizationId: input.authorizationId, launchMode: 'directory',
    }))
      .rejects.toThrow('directory_authorization_invalid')
  })

  it('retains and consumes a direct-directory grant when creation is unknown', async () => {
    const request = vi.fn(async () => ({ path: '/project/direct', name: 'direct' }))
    const { backend } = createBackend({
      sessionDirectoryRequest: request,
      hostSetup: (host, index) => {
        if (index === 1) host.start.mockRejectedValueOnce(new Error(
          'desktop_session_creation_state_unknown: Host failed after ALIVE',
        ))
      },
    })
    await backend.bootstrap()
    const selected = await backend.authorizeSessionDirectory('/project/direct')
    const input = {
      authorizationId: selected.authorizationId, launchMode: 'directory' as const,
    }

    await expect(backend.createSession(input)).rejects.toThrow(
      'directory_retained: session_creation_state_unknown',
    )
    expect((await backend.bootstrap()).workspaces).toContainEqual(expect.objectContaining({
      rootUri: 'file:///project/direct', kind: 'folder',
    }))
    await expect(backend.createSession(input)).rejects.toThrow('directory_authorization_invalid')
  })
})
