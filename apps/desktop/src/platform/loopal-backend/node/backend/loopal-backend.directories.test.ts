import { mkdir, mkdtemp, readFile, realpath, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { createBackend } from './loopal-backend.test-fixtures'
import { nativeTestFileUri, nativeTestPath } from './loopal-backend.test-paths'

describe('LoopalDesktopBackend session directories', () => {
  it('routes two directories through isolated workspace IDs and preserves cwd on restart', async () => {
    const inspect = vi.fn(async (_method: string, input: unknown) => {
      const path = (input as { path: string }).path
      return { path, name: path.split('/').at(-1) }
    })
    const { backend, inputs } = createBackend({ sessionDirectoryRequest: inspect })
    await backend.bootstrap()
    const firstSelection = await backend.authorizeSessionDirectory('/projects/one')
    const first = await backend.createSession({
      authorizationId: firstSelection.authorizationId, launchMode: 'directory',
    })
    const secondSelection = await backend.authorizeSessionDirectory('/projects/two')
    const second = await backend.createSession({
      authorizationId: secondSelection.authorizationId, launchMode: 'directory',
    })
    expect(first.session.workspaceId).not.toBe(second.session.workspaceId)
    expect((await backend.bootstrap()).workspaces).toEqual(expect.arrayContaining([
      expect.objectContaining({ rootUri: nativeTestFileUri('/projects/one') }),
      expect.objectContaining({ rootUri: nativeTestFileUri('/projects/two') }),
    ]))
    await backend.stopSession(first.session.id)
    await backend.restartSession(first.session.id)
    expect(inputs.at(-1)).toMatchObject({
      workspaceId: first.session.workspaceId,
      cwd: nativeTestPath('/projects/one'), resumeSessionId: first.session.id,
    })
  })

  it('authorizes and creates after every live Session was stopped', async () => {
    const { backend, inputs } = createBackend({
      sessionDirectoryRequest: async () => ({ path: '/projects/new', name: 'new' }),
    })
    await backend.bootstrap()
    await backend.stopSession('session-1')
    const selected = await backend.authorizeSessionDirectory('/projects/new')
    expect(selected.path).toBe('/projects/new')
    await backend.createSession({
      authorizationId: selected.authorizationId, launchMode: 'directory',
    })
    expect(inputs.at(-1)?.cwd).toBe(selected.path)
  })

  it('restores a selected session cwd and workspace across app relaunch', async () => {
    const root = await mkdtemp(join(tmpdir(), 'loopal-session-cwd-'))
    const state = join(root, 'state.json')
    const projectPath = join(root, 'persisted')
    try {
      await mkdir(projectPath)
      const project = await realpath(projectPath)
      const request = async () => ({ path: project, name: 'persisted' })
      const first = createBackend({ sessionStatePath: state, sessionDirectoryRequest: request })
      await first.backend.bootstrap()
      const selected = await first.backend.authorizeSessionDirectory(project)
      const created = await first.backend.createSession({
        authorizationId: selected.authorizationId, launchMode: 'directory',
      })
      await first.backend.shutdown()

      const second = createBackend({ sessionStatePath: state, sessionDirectoryRequest: request })
      const restored = await second.backend.bootstrap()
      expect(second.inputs[0]).toMatchObject({
        cwd: project, resumeSessionId: created.session.id,
      })
      expect(restored.workspaces).toContainEqual(expect.objectContaining({
        id: created.session.workspaceId,
        rootUri: nativeTestFileUri(project),
      }))
      await second.backend.shutdown()
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })

  it('keeps a missing persisted session visible without relaunching Loopal in its cwd', async () => {
    const root = await mkdtemp(join(tmpdir(), 'loopal-session-missing-cwd-'))
    const state = join(root, 'state.json')
    const projectPath = join(root, 'moved-project')
    try {
      await mkdir(projectPath)
      const project = await realpath(projectPath)
      const request = async () => ({ path: project, name: 'moved-project' })
      const first = createBackend({ sessionStatePath: state, sessionDirectoryRequest: request })
      await first.backend.bootstrap()
      await first.backend.stopSession('session-1')
      const selected = await first.backend.authorizeSessionDirectory(project)
      const created = await first.backend.createSession({
        authorizationId: selected.authorizationId, launchMode: 'directory',
      })
      await first.backend.shutdown()
      await rm(project, { recursive: true })

      const second = createBackend({ sessionStatePath: state, sessionDirectoryRequest: request })
      const restored = await second.backend.bootstrap()
      expect(second.inputs[0]).toEqual({
        workspaceId: 'local-workspace', cwd: nativeTestPath('/workspace/project'),
      })
      expect(restored.sessions).toContainEqual(expect.objectContaining({
        id: created.session.id, status: 'failed', attention: 'failure',
      }))
      expect(JSON.parse(await readFile(state, 'utf8'))).toMatchObject({
        activeSessionId: restored.activeSessionId,
        runningSessionIds: [restored.activeSessionId],
      })
      await second.backend.shutdown()
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })
})
