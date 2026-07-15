import { buildFallbackContext } from './fallback-context'
import { sessionOne } from '../../../test/support/workbench/api-stub'

describe('buildFallbackContext', () => {
  it('projects active workspaces, sessions, and matching runtime generations', () => {
    const context = buildFallbackContext({
      workspaces: [{ id: 'workspace', name: 'Loopal', rootUri: '/loopal', kind: 'folder' }],
      sessions: [sessionOne, { ...sessionOne, id: 'stopped', activeRuntimeId: undefined }],
      runtimes: [{
        id: 'runtime-1', sessionId: sessionOne.id, workspaceId: 'workspace', generation: 3,
        state: 'ready', rootAgent: 'main',
      }],
      activeWorkspaceId: 'workspace', activeSessionId: sessionOne.id,
    })
    expect(context.workspaces).toEqual([{ id: 'workspace', name: 'Loopal', detail: '/loopal' }])
    expect(context.sessions[0]).toMatchObject({ runtimeId: 'runtime-1', runtimeGeneration: 3 })
    expect(context.sessions[1]).not.toHaveProperty('runtimeId')
    expect(context).toMatchObject({ activeWorkspaceId: 'workspace', activeSessionId: sessionOne.id })
  })

  it('omits absent active selections', () => {
    expect(buildFallbackContext({ workspaces: [], sessions: [], runtimes: [] })).toEqual({
      workspaces: [], sessions: [],
    })
  })
})
