import { act, renderHook, waitFor } from '@testing-library/react'
import {
  createTestAPI, sessionOne, sessionTwo, updatedAt,
} from '../../../../../test/support/workbench/api-stub'
import {
  type LocalMetaHubStatus, type MetaHubRuntimeState, type MetaHubSettings,
} from '../../../../shared/contracts'
import { useFederationController } from './use-federation-controller'

const runtimes = [
  { id: 'runtime-1', sessionId: sessionOne.id, workspaceId: 'workspace', generation: 1,
    state: 'ready' as const, rootAgent: 'main' },
  { id: 'runtime-2', sessionId: sessionTwo.id, workspaceId: 'workspace', generation: 1,
    state: 'ready' as const, rootAgent: 'main' },
]
const sessions = [sessionOne, sessionTwo]
const disconnected = (): MetaHubRuntimeState => ({
  state: 'disconnected', hubs: [], topology: [], refreshedAt: updatedAt,
})
const connected = (address: string, hubName: string): MetaHubRuntimeState => ({
  state: 'connected', address, hubName,
  hubs: [{ name: hubName, status: 'connected', agentCount: 1, capabilities: [] }],
  topology: [{ id: `${hubName}/main`, name: 'main', hub: hubName,
    hubPath: [hubName], children: [], lifecycle: 'running' }],
  refreshedAt: updatedAt,
})

describe('useFederationController', () => {
  it('starts and persists the coordinator, then joins and leaves sessions independently', async () => {
    let local: LocalMetaHubStatus = { state: 'stopped' }
    let settings: MetaHubSettings = {
      address: '', hubName: 'desktop-test', joinOnStart: true,
      startLocalOnLaunch: false, tokenConfigured: false,
    }
    const states = new Map<string, MetaHubRuntimeState>()
    const update = vi.fn(async (input) => {
      settings = { ...input, tokenConfigured: true }
      return settings
    })
    const join = vi.fn(async (input) => {
      const state: MetaHubRuntimeState = {
        state: 'connected', address: settings.address, hubName: input.hubName,
        hubs: [{ name: input.hubName!, status: 'connected', agentCount: 1,
          capabilities: ['desktop'] }],
        topology: [{ id: `${input.hubName}/main`, name: 'main', hub: input.hubName!,
          hubPath: [input.hubName!], children: [], lifecycle: 'running' }],
        refreshedAt: updatedAt,
      }
      states.set(input.sessionId, state)
      return state
    })
    const leave = vi.fn(async (target) => {
      const state = disconnected(); states.set(target.sessionId, state); return state
    })
    const { api } = createTestAPI({
      getMetaHubSettings: async () => settings,
      updateMetaHubSettings: update,
      getLocalMetaHubStatus: async () => local,
      startLocalMetaHub: async () => {
        const address = '127.0.0.1:39000'
        local = { state: 'running', address }
        settings = { ...settings, address, tokenConfigured: true }
        return local
      },
      getMetaHubStatus: async (target) => states.get(target.sessionId) ?? disconnected(),
      joinMetaHub: join, disconnectMetaHub: leave,
    })
    const hook = renderHook(() => useFederationController(
      api, sessions, runtimes,
    ))
    await waitFor(() => expect(hook.result.current.settings).toBeDefined())
    await act(() => hook.result.current.start())
    expect(update).toHaveBeenCalledWith(expect.objectContaining({
      startLocalOnLaunch: true, joinOnStart: false,
    }))
    expect(join).not.toHaveBeenCalled()
    expect(hook.result.current.snapshot.local.state).toBe('running')

    await act(() => hook.result.current.join(sessionOne.id))
    await act(() => hook.result.current.join(sessionTwo.id))
    expect(join).toHaveBeenCalledTimes(2)
    expect(join.mock.calls[0]![0].hubName).not.toBe(join.mock.calls[1]![0].hubName)
    expect(hook.result.current.snapshot.memberships).toEqual({
      [sessionOne.id]: 'connected', [sessionTwo.id]: 'connected',
    })
    await act(() => hook.result.current.leave(sessionOne.id))
    expect(leave).toHaveBeenCalledWith(expect.objectContaining({ sessionId: sessionOne.id }))
    expect(hook.result.current.snapshot.memberships).toEqual({
      [sessionOne.id]: 'disconnected', [sessionTwo.id]: 'connected',
    })
  })

  it('drops a status response from an obsolete runtime generation', async () => {
    let release: ((state: MetaHubRuntimeState) => void) | undefined
    const old = new Promise<MetaHubRuntimeState>((resolve) => { release = resolve })
    const getStatus = vi.fn((target) => target.generation === 1
      ? old : Promise.resolve(disconnected()))
    const { api } = createTestAPI({ getMetaHubStatus: getStatus })
    const hook = renderHook(
      ({ sessions, values }) => useFederationController(api, sessions, values),
      { initialProps: { sessions: [sessionOne], values: [runtimes[0]!] } },
    )
    const nextSession = { ...sessionOne, activeRuntimeId: 'runtime-new' }
    const nextRuntime = { ...runtimes[0]!, id: 'runtime-new', generation: 2 }
    hook.rerender({ sessions: [nextSession], values: [runtimes[0]!, nextRuntime] })
    await waitFor(() => expect(getStatus).toHaveBeenCalledWith(expect.objectContaining({
      runtimeId: 'runtime-new', generation: 2,
    })))
    await act(async () => release?.({
      state: 'connected', hubs: [], topology: [], refreshedAt: updatedAt,
    }))
    expect(hook.result.current.snapshot.memberships[sessionOne.id]).toBe('unavailable')
  })

  it('uses configured address and excludes an externally connected session', async () => {
    const address = 'configured:9000'
    const { api } = createTestAPI({
      getMetaHubSettings: async () => ({
        address, hubName: 'desktop-test', joinOnStart: false,
        startLocalOnLaunch: true, tokenConfigured: true,
      }),
      getLocalMetaHubStatus: async () => ({ state: 'running', address: 'managed:9000' }),
      getMetaHubStatus: async (target) => target.sessionId === sessionOne.id
        ? connected(address, 'current') : connected('external:9000', 'external'),
    })
    const hook = renderHook(() => useFederationController(api, sessions, runtimes))
    await waitFor(() => expect(hook.result.current.snapshot.memberships).toEqual({
      [sessionOne.id]: 'connected', [sessionTwo.id]: 'external',
    }))
    expect(hook.result.current.snapshot.network.address).toBe(address)
    expect(hook.result.current.snapshot.network.hubs.map(({ name }) => name)).toEqual(['current'])
    expect(hook.result.current.snapshot.connections).toHaveLength(1)
  })

  it('refreshes partial mutations after failure and preserves the operation error', async () => {
    const address = 'configured:9000'
    let state = connected(address, 'partially-left')
    const { api } = createTestAPI({
      getMetaHubSettings: async () => ({
        address, hubName: 'desktop-test', joinOnStart: false,
        startLocalOnLaunch: false, tokenConfigured: true,
      }),
      getMetaHubStatus: async () => state,
      disconnectMetaHub: async () => {
        state = disconnected()
        throw new Error('leave acknowledgement lost')
      },
    })
    const oneSession = [sessionOne]
    const oneRuntime = [runtimes[0]!]
    const hook = renderHook(() => useFederationController(api, oneSession, oneRuntime))
    await waitFor(() => expect(
      hook.result.current.snapshot.memberships[sessionOne.id],
    ).toBe('connected'))
    await act(() => hook.result.current.leave(sessionOne.id))
    await waitFor(() => expect(
      hook.result.current.snapshot.memberships[sessionOne.id],
    ).toBe('disconnected'))
    expect(hook.result.current.snapshot.network.hubs).toEqual([])
    await waitFor(() => expect(hook.result.current.error).toBe('leave acknowledgement lost'))
  })
})
