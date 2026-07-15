import { mkdir, mkdtemp, readFile, readdir, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { LoopalSessionResumeState } from './loopal-session-resume-state'

describe('LoopalSessionResumeState', () => {
  let root = ''
  let path = ''

  beforeEach(async () => {
    root = await mkdtemp(join(tmpdir(), 'loopal-desktop-state-'))
    path = join(root, 'nested', 'lifecycle.json')
  })

  afterEach(async () => rm(root, { recursive: true, force: true }))

  it('persists the selected running sessions and explicit stops atomically', async () => {
    const state = new LoopalSessionResumeState('/workspace', path)
    await state.load()
    await state.started('session-a', true)
    await state.started('session-b', false)
    await state.select('session-b')
    expect(state.resumeSessionId).toBe('session-b')
    await state.stopped('session-b')
    await state.flush()
    expect(state.activeSessionId).toBe('session-b')
    expect(state.resumeSessionId).toBe('session-a')

    const restored = new LoopalSessionResumeState('/workspace', path)
    await restored.load()
    expect(restored.activeSessionId).toBe('session-b')
    expect(restored.resumeSessionId).toBe('session-a')
    expect((await readdir(join(root, 'nested'))).filter((name) => name.endsWith('.tmp')))
      .toEqual([])
  })

  it('ignores missing, corrupt, and foreign-workspace records', async () => {
    const missing = new LoopalSessionResumeState('/workspace', path)
    await expect(missing.load()).resolves.toBeUndefined()
    expect(missing.resumeSessionId).toBeUndefined()

    await mkdir(join(root, 'nested'))
    await writeFile(path, '{broken')
    const corrupt = new LoopalSessionResumeState('/workspace', path)
    await corrupt.load()
    expect(corrupt.activeSessionId).toBeUndefined()

    await writeFile(path, JSON.stringify({
      version: 1, workspace: '/other', activeSessionId: 'foreign',
      runningSessionIds: ['foreign'],
    }))
    const foreign = new LoopalSessionResumeState('/workspace', path)
    await foreign.load()
    expect(foreign.resumeSessionId).toBeUndefined()
  })

  it('records a created location without claiming that the Session is running', async () => {
    const state = new LoopalSessionResumeState('/workspace', path)
    const location = {
      sessionId: 'session-created', workspaceId: 'workspace-created',
      cwd: '/workspace/created', name: 'created', kind: 'git_worktree' as const,
    }
    await state.created(location)
    expect(state.location(location.sessionId)).toEqual(location)
    expect(state.runningSessionIds).not.toContain(location.sessionId)
    expect(state.resumeSessionId).toBeUndefined()

    const restored = new LoopalSessionResumeState('/workspace', path)
    await restored.load()
    expect(restored.location(location.sessionId)).toEqual(location)
    expect(restored.resumeSessionId).toBeUndefined()
    await restored.started(location.sessionId, true, location)
    expect(restored.resumeSessionId).toBe(location.sessionId)
  })

  it('works in memory and bounds persisted runtime candidates', async () => {
    const state = new LoopalSessionResumeState('/workspace')
    await state.load()
    for (let index = 0; index < 40; index += 1) {
      await state.started(`session-${index}`, index === 0)
    }
    await state.select('not-running')
    expect(state.resumeSessionId).toBe('session-39')
    await expect(state.flush()).resolves.toBeUndefined()
    await expect(readFile(path)).rejects.toThrow()
  })
})
