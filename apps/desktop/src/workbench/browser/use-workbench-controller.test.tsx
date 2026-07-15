import { act, renderHook, waitFor } from '@testing-library/react'
import {
  createTestAPI, sessionDetail, sessionOne,
} from '../../../test/support/workbench/api-stub'
import { useWorkbenchController } from './use-workbench-controller'

describe('useWorkbenchController session lifecycle guards', () => {
  const createInput = {
    authorizationId: 'd10f67f2-f471-44ea-b6d1-e1b963e11228',
    launchMode: 'directory' as const,
  }
  it('creates without a workspace while runtime lifecycle needs a selected session', async () => {
    const createSession = vi.fn(async () => sessionDetail(sessionOne))
    const stopSession = vi.fn(async () => undefined)
    const restartSession = vi.fn()
    const { api } = createTestAPI({
      bootstrap: async () => ({
        protocolVersion: 2, hostStatus: 'stopped', workspaces: [],
        sessions: [], runtimes: [],
      }),
      createSession, stopSession, restartSession,
    })
    const hook = renderHook(() => useWorkbenchController(api))
    await waitFor(() => expect(hook.result.current.projection.hostStatus).toBe('stopped'))
    await act(async () => {
      await hook.result.current.stopSession()
      await hook.result.current.restartSession()
    })
    expect(stopSession).not.toHaveBeenCalled()
    expect(restartSession).not.toHaveBeenCalled()
    expect(hook.result.current.canCreate).toBe(true)
    await act(async () => { await hook.result.current.createSession(createInput) })
    expect(createSession).toHaveBeenCalledWith(createInput)
  })

  it('reports create and runtime lifecycle failures', async () => {
    const createSession = vi.fn(async () => { throw new Error('create failed') })
    const stopSession = vi.fn(async () => { throw new Error('stop failed') })
    const restartSession = vi.fn(async () => { throw new Error('restart failed') })
    const { api } = createTestAPI({
      bootstrap: async () => ({
        protocolVersion: 2, hostStatus: 'ready',
        workspaces: [{
          id: 'workspace', name: 'Loopal', rootUri: '/loopal', kind: 'folder',
        }],
        sessions: [sessionOne], runtimes: [], activeSessionId: sessionOne.id,
      }),
      createSession, stopSession, restartSession,
    })
    const hook = renderHook(() => useWorkbenchController(api))
    await waitFor(() => expect(hook.result.current.activeSessionId).toBe(sessionOne.id))
    await act(async () => { await hook.result.current.createSession(createInput) })
    expect(hook.result.current.error).toBe('create failed')
    await act(async () => hook.result.current.stopSession())
    expect(hook.result.current.error).toBe('stop failed')
    await act(async () => hook.result.current.restartSession())
    expect(hook.result.current.error).toBe('restart failed')
  })

  it('refreshes dynamic workspaces and selects the created session', async () => {
    const initial = {
      id: 'workspace', name: 'Loopal', rootUri: '/loopal', kind: 'folder' as const,
    }
    const selected = {
      id: 'local-new', name: 'feature', rootUri: '/work/feature', kind: 'git_worktree' as const,
    }
    const created = {
      ...sessionOne, id: 'session-new', workspaceId: selected.id, title: 'Feature session',
    }
    const bootstrap = vi.fn()
      .mockResolvedValueOnce({
        protocolVersion: 2, hostStatus: 'ready', workspaces: [initial],
        sessions: [sessionOne], runtimes: [], activeSessionId: sessionOne.id,
      })
      .mockResolvedValue({
        protocolVersion: 2, hostStatus: 'ready', workspaces: [initial, selected],
        sessions: [sessionOne, created], runtimes: [], activeSessionId: created.id,
      })
    const { api } = createTestAPI({
      bootstrap, createSession: async () => sessionDetail(created),
    })
    const hook = renderHook(() => useWorkbenchController(api))
    await waitFor(() => expect(hook.result.current.activeSessionId).toBe(sessionOne.id))
    await act(async () => { await hook.result.current.createSession(createInput) })

    expect(hook.result.current.activeWorkspaceId).toBe(selected.id)
    expect(hook.result.current.workspaces).toContainEqual(selected)
    expect(hook.result.current.currentSessions.map(({ id }) => id))
      .toEqual(['session-1', 'session-new'])
    expect(hook.result.current.searchResults).toEqual([])
    expect(bootstrap).toHaveBeenCalledTimes(2)
  })

  it('does not auto-open history but keeps it available to cross-workspace search', async () => {
    const history = {
      ...sessionOne, id: 'history', workspaceId: 'workspace-history',
      title: 'Legacy desktop audit', status: 'stopped' as const,
      activeRuntimeId: undefined,
    }
    const openSession = vi.fn(async () => sessionDetail(history))
    const { api } = createTestAPI({
      bootstrap: async () => ({
        protocolVersion: 2, hostStatus: 'ready', workspaces: [],
        sessions: [history], runtimes: [],
      }),
      openSession,
    })
    const hook = renderHook(() => useWorkbenchController(api))
    await waitFor(() => expect(hook.result.current.projection.hostStatus).toBe('ready'))
    expect(hook.result.current.activeSessionId).toBeUndefined()
    expect(hook.result.current.activeWorkspaceId).toBeUndefined()
    expect(hook.result.current.currentSessions).toEqual([])
    expect(openSession).not.toHaveBeenCalled()
    act(() => hook.result.current.setQuery('desktop'))
    expect(hook.result.current.searchResults.map(({ id }) => id)).toEqual(['history'])
  })

  it('honors an explicit active historical session and restores its workspace scope', async () => {
    const history = {
      ...sessionOne, id: 'history-active', workspaceId: 'workspace-history',
      status: 'stopped' as const, activeRuntimeId: undefined,
    }
    const openSession = vi.fn(async () => sessionDetail(history))
    const { api } = createTestAPI({
      bootstrap: async () => ({
        protocolVersion: 2, hostStatus: 'ready', workspaces: [],
        sessions: [history], runtimes: [], activeSessionId: history.id,
      }),
      openSession,
    })
    const hook = renderHook(() => useWorkbenchController(api))
    await waitFor(() => expect(hook.result.current.activeSessionId).toBe(history.id))
    expect(openSession).toHaveBeenCalledWith(history.id)
    expect(hook.result.current.activeWorkspaceId).toBe(history.workspaceId)
  })

  it('opens a detail even when an event adds no matching summary first', async () => {
    const openSession = vi.fn(async () => sessionDetail(sessionOne))
    const { api } = createTestAPI({ openSession })
    const hook = renderHook(() => useWorkbenchController(api))
    await waitFor(() => expect(hook.result.current.activeSessionId).toBe(sessionOne.id))
    await act(async () => hook.result.current.openSession('unknown'))
    expect(openSession).toHaveBeenLastCalledWith('unknown')
  })
})
