import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { createBackend } from './loopal-backend.test-fixtures'
import { nativeTestPath } from './loopal-backend.test-paths'

describe('LoopalDesktopBackend persisted runtime intent', () => {
  let root = ''
  let statePath = ''

  beforeEach(async () => {
    root = await mkdtemp(join(tmpdir(), 'loopal-backend-resume-'))
    statePath = join(root, 'lifecycle.json')
  })

  afterEach(async () => rm(root, { recursive: true, force: true }))

  it('explicitly resumes the last live session during bootstrap', async () => {
    await seed(statePath, 'session-2')
    const { backend, inputs, hosts } = createBackend({ sessionStatePath: statePath })

    const bootstrap = await backend.bootstrap()
    expect(inputs).toEqual([{
      workspaceId: 'local-workspace', cwd: nativeTestPath('/workspace/project'),
      resumeSessionId: 'session-2',
    }])
    expect(bootstrap.activeSessionId).toBe('session-2')
    expect(bootstrap.sessions).toContainEqual(expect.objectContaining({
      id: 'session-2', status: 'waiting', activeRuntimeId: 'runtime-1',
    }))
    await expect(backend.openSession('session-2')).resolves.toMatchObject({
      conversation: [expect.objectContaining({ text: 'Answer from session-2' })],
    })
    expect(hosts).toHaveLength(1)
  })

  it('clears an invalid resume intent and falls back to a fresh Host', async () => {
    await seed(statePath, 'missing-session')
    const { backend, inputs } = createBackend({
      sessionStatePath: statePath,
      hostSetup: (host, index) => {
        if (index === 0) host.start.mockRejectedValueOnce(new Error('persisted session missing'))
      },
    })

    const bootstrap = await backend.bootstrap()
    expect(inputs).toEqual([
      {
        workspaceId: 'local-workspace', cwd: nativeTestPath('/workspace/project'),
        resumeSessionId: 'missing-session',
      },
      { workspaceId: 'local-workspace', cwd: nativeTestPath('/workspace/project') },
    ])
    expect(bootstrap.activeSessionId).toBe('session-1')
    expect(JSON.parse(await readFile(statePath, 'utf8'))).toMatchObject({
      activeSessionId: 'session-1', runningSessionIds: ['session-1'],
    })
  })

  it('normalizes persisted running intent to the runtime actually recovered', async () => {
    await writeFile(statePath, JSON.stringify({
      version: 1, workspace: nativeTestPath('/workspace/project'), activeSessionId: 'session-2',
      runningSessionIds: ['session-2', 'session-1'],
    }))
    const { backend } = createBackend({ sessionStatePath: statePath })

    await backend.bootstrap()

    expect(JSON.parse(await readFile(statePath, 'utf8'))).toMatchObject({
      activeSessionId: 'session-2', runningSessionIds: ['session-2'],
    })
  })
})

async function seed(path: string, sessionId: string): Promise<void> {
  await writeFile(path, JSON.stringify({
    version: 1, workspace: nativeTestPath('/workspace/project'), activeSessionId: sessionId,
    runningSessionIds: [sessionId],
  }))
}
