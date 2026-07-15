import { CancellationToken } from '../../../../base/common/cancellation'
import { createBackendStub } from '../../../../../test/support/backend/backend-stub'
import { DesktopBackendChannel } from './backend-channel'

const token = CancellationToken.None

describe('DesktopBackendChannel code workbench commands', () => {
  it('dispatches workspace, Git, worktree, and attention commands', async () => {
    const backend = createBackendStub()
    const channel = new DesktopBackendChannel(backend)
    const call = (command: string, input: unknown) => channel.call({}, command, input, token)

    await expect(call('listDirectory', { workspaceId: 'workspace', path: '' }))
      .resolves.toMatchObject({ workspaceId: 'workspace' })
    await expect(call('readFile', { workspaceId: 'workspace', path: 'a.ts' }))
      .resolves.toMatchObject({ version: 'v1' })
    await expect(call('writeFile', {
      workspaceId: 'workspace', path: 'a.ts', content: 'next', expectedVersion: 'v1',
    })).resolves.toMatchObject({ content: 'next', version: 'v2' })
    await expect(call('searchWorkspace', { workspaceId: 'workspace', query: 'next' }))
      .resolves.toEqual({ matches: [], truncated: false })
    await expect(call('gitStatus', { workspaceId: 'workspace' }))
      .resolves.toMatchObject({ branch: 'main' })
    await expect(call('gitDiff', { workspaceId: 'workspace', path: 'a.ts' }))
      .resolves.toMatchObject({ path: 'a.ts' })
    await expect(call('gitStage', { workspaceId: 'workspace', path: 'a.ts' }))
      .resolves.toBeUndefined()
    await expect(call('gitUnstage', { workspaceId: 'workspace', path: 'a.ts' }))
      .resolves.toBeUndefined()
    await expect(call('listWorktrees', { workspaceId: 'workspace' })).resolves.toEqual([])
    await expect(call('createWorktree', { workspaceId: 'workspace', name: 'feature' }))
      .resolves.toMatchObject({ id: 'feature' })
    await expect(call('removeWorktree', {
      workspaceId: 'workspace', name: 'feature', force: false,
    })).resolves.toBeUndefined()
    const scope = {
      sessionId: 'session', runtimeId: 'runtime', generation: 1, agentId: 'main',
    }
    await expect(call('respondPermission', {
      ...scope, requestId: 'p1', decision: 'allow_once',
    }))
      .resolves.toBeUndefined()
    await expect(call('respondQuestion', { ...scope, requestId: 'q1', answers: ['yes'] }))
      .resolves.toBeUndefined()
    await expect(call('respondQuestion', { ...scope, requestId: 'q2', cancelled: true }))
      .resolves.toBeUndefined()

    expect(backend.searchWorkspace).toHaveBeenCalledWith(
      { workspaceId: 'workspace', query: 'next', maxResults: 200 }, token,
    )
    expect(backend.gitStage).toHaveBeenCalledWith(
      { workspaceId: 'workspace', path: 'a.ts' }, token,
    )
    expect(backend.respondQuestion).toHaveBeenLastCalledWith(
      { ...scope, requestId: 'q2', cancelled: true }, token,
    )
  })

  it('rejects unsafe paths and malformed worktree or attention inputs', async () => {
    const channel = new DesktopBackendChannel(createBackendStub())
    await expect(channel.call({}, 'readFile', {
      workspaceId: 'workspace', path: '../secret',
    }, token)).rejects.toThrow('path must stay inside its workspace')
    await expect(channel.call({}, 'createWorktree', {
      workspaceId: 'workspace', name: '../escape',
    }, token)).rejects.toThrow()
    await expect(channel.call({}, 'respondQuestion', {
      sessionId: 's', runtimeId: 'r', generation: 1, agentId: 'main', requestId: 'q',
    }, token)).rejects.toThrow('Provide answers or cancel')
  })
})
