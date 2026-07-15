import { fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { createTestAPI, updatedAt } from '../../../../../test/support/workbench/api-stub'
import { type JoinMetaHubInput } from '../../../../shared/contracts'
import { federationHubName } from '../../../../shared/contracts/metahub-identity'
import { MetaHubSettingsSection } from './metahub-settings-section'
import { MetaHubTopologyPanel } from '../../federation/browser/metahub-topology-panel'

const target = { sessionId: 'session-1', runtimeId: 'runtime-1', generation: 1 }
const restartedTarget = { sessionId: 'session-1', runtimeId: 'runtime-2', generation: 2 }
const connected = {
  state: 'connected' as const,
  address: '127.0.0.1:39000',
  hubName: 'desktop-ui',
  hubs: [{
    name: 'desktop-ui', status: 'connected' as const, agentCount: 1, capabilities: ['desktop'],
  }],
  topology: [{
    id: 'desktop-ui/main', name: 'main', hub: 'desktop-ui', hubPath: ['desktop-ui'],
    children: [], lifecycle: 'running' as const,
  }],
  refreshedAt: updatedAt,
}

describe('MetaHubSettingsSection', () => {
  it('keeps persisted settings when runtime status refresh fails', async () => {
    const { api } = createTestAPI({
      getMetaHubSettings: async () => ({
        address: 'meta.internal:9000', hubName: 'persisted', joinOnStart: true,
        startLocalOnLaunch: false, tokenConfigured: true,
      }),
      getMetaHubStatus: async () => { throw new Error('status unavailable') },
    })
    render(<MetaHubSettingsSection api={api} target={target} />)
    expect(await screen.findByDisplayValue('meta.internal:9000')).toBeInTheDocument()
    expect(screen.getByDisplayValue('persisted')).toBeInTheDocument()
    expect(screen.getByRole('alert')).toHaveTextContent('status unavailable')
  })

  it('saves secrets, starts local, joins, refreshes, disconnects, clears, and stops', async () => {
    let localRunning = false
    let settings = {
      address: '', hubName: 'desktop-ui', joinOnStart: false,
      startLocalOnLaunch: false, tokenConfigured: false,
    }
    const update = vi.fn(async (input) => {
      settings = {
        address: input.address,
        hubName: input.hubName,
        joinOnStart: input.joinOnStart,
        startLocalOnLaunch: input.startLocalOnLaunch,
        tokenConfigured: input.clearToken ? false : Boolean(input.token || settings.tokenConfigured),
      }
      return settings
    })
    const start = vi.fn(async () => {
      localRunning = true
      settings = { ...settings, address: '127.0.0.1:39000', tokenConfigured: true }
      return { state: 'running' as const, address: settings.address }
    })
    const join = vi.fn(async (_input: JoinMetaHubInput) => connected)
    const disconnect = vi.fn(async () => ({
      state: 'disconnected' as const, hubs: [], topology: [], refreshedAt: updatedAt,
    }))
    const stop = vi.fn(async () => {
      localRunning = false
      return { state: 'stopped' as const }
    })
    const refresh = vi.fn()
      .mockResolvedValueOnce({
        state: 'disconnected' as const, hubs: [], topology: [], refreshedAt: updatedAt,
      })
      .mockResolvedValue(connected)
    const { api } = createTestAPI({
      getMetaHubSettings: async () => settings,
      updateMetaHubSettings: update,
      getLocalMetaHubStatus: async () => localRunning
        ? ({ state: 'running', address: '127.0.0.1:39000' })
        : ({ state: 'stopped' }),
      getMetaHubStatus: refresh,
      startLocalMetaHub: start, joinMetaHub: join,
      disconnectMetaHub: disconnect, stopLocalMetaHub: stop,
    })
    const view = render(<MetaHubSettingsSection api={api} target={target} />)
    const panel = await screen.findByTestId('metahub-settings')
    fireEvent.change(within(panel).getByLabelText('MetaHub address'), {
      target: { value: 'meta.internal:9000' },
    })
    fireEvent.change(within(panel).getByLabelText('MetaHub token'), {
      target: { value: 'private-value' },
    })
    fireEvent.click(within(panel).getByLabelText('Join MetaHub on session start'))
    fireEvent.click(within(panel).getByRole('button', { name: 'Save' }))
    await waitFor(() => expect(update).toHaveBeenCalled())
    expect(within(panel).getByLabelText('MetaHub token')).toHaveValue('')

    fireEvent.click(within(panel).getByRole('button', { name: 'Start local & join' }))
    await waitFor(() => expect(join).toHaveBeenCalledWith({
      ...target, hubName: federationHubName('desktop-ui', target),
    }))
    expect(within(panel).getByTestId('metahub-topology')).toHaveTextContent('desktop-ui/main')
    view.rerender(<MetaHubSettingsSection api={api} target={restartedTarget} />)
    await waitFor(() => expect(refresh).toHaveBeenCalledTimes(2))
    fireEvent.click(within(panel).getByRole('button', { name: 'Join / Reconnect' }))
    await waitFor(() => expect(join).toHaveBeenCalledWith({
      ...restartedTarget, hubName: federationHubName('desktop-ui', restartedTarget),
    }))
    expect(join.mock.calls[0]?.[0].hubName).not.toBe(join.mock.calls[1]?.[0].hubName)
    fireEvent.click(within(panel).getByRole('button', { name: 'Refresh' }))
    await waitFor(() => expect(refresh).toHaveBeenCalledTimes(3))
    fireEvent.click(within(panel).getByRole('button', { name: 'Disconnect' }))
    await waitFor(() => expect(disconnect).toHaveBeenCalled())
    fireEvent.click(within(panel).getByRole('button', { name: 'Clear stored token' }))
    await waitFor(() => expect(update).toHaveBeenLastCalledWith(expect.objectContaining({
      clearToken: true,
    })))
    fireEvent.click(within(panel).getByRole('button', { name: 'Stop local MetaHub' }))
    await waitFor(() => expect(stop).toHaveBeenCalled())
  })

  it('does not disconnect an external uplink when stopping the local coordinator', async () => {
    const disconnect = vi.fn()
    const stop = vi.fn(async () => ({ state: 'stopped' as const }))
    const { api } = createTestAPI({
      getLocalMetaHubStatus: async () => ({ state: 'running', address: '127.0.0.1:39000' }),
      getMetaHubStatus: async () => ({ ...connected, address: 'meta.external:9000' }),
      disconnectMetaHub: disconnect,
      stopLocalMetaHub: stop,
    })
    render(<MetaHubSettingsSection api={api} target={target} initialState={connected} />)
    fireEvent.click(await screen.findByRole('button', { name: 'Stop local MetaHub' }))
    await waitFor(() => expect(stop).toHaveBeenCalled())
    expect(disconnect).not.toHaveBeenCalled()
  })

  it('can clear or restart a failed managed coordinator', async () => {
    const stop = vi.fn(async () => ({ state: 'stopped' as const }))
    const { api } = createTestAPI({
      getLocalMetaHubStatus: async () => ({ state: 'failed', error: 'child crashed' }),
      stopLocalMetaHub: stop,
    })
    render(<MetaHubSettingsSection api={api} />)
    expect(await screen.findByRole('button', { name: 'Restart local & join' })).toBeInTheDocument()
    fireEvent.click(await screen.findByRole('button', { name: 'Clear failed local MetaHub' }))
    await waitFor(() => expect(stop).toHaveBeenCalled())
  })
})

describe('MetaHubTopologyPanel', () => {
  it('renders disconnected, connected, and error states', () => {
    const view = render(<MetaHubTopologyPanel state={{
      state: 'disconnected', hubs: [], topology: [], refreshedAt: updatedAt,
    }} />)
    expect(screen.getByText(/Join a MetaHub/)).toBeInTheDocument()
    view.rerender(<MetaHubTopologyPanel state={connected} />)
    expect(screen.getByText('desktop-ui/main')).toBeInTheDocument()
    view.rerender(<MetaHubTopologyPanel state={{
      state: 'error', hubs: [], topology: [], error: 'cluster failed', refreshedAt: updatedAt,
    }} />)
    expect(screen.getByText('cluster failed')).toBeInTheDocument()
  })

  it('filters Hubs, selects remote Agents, and opens management', () => {
    const onSelectAgent = vi.fn()
    const onManage = vi.fn()
    render(<MetaHubTopologyPanel
      state={{
        ...connected,
        hubs: [...connected.hubs, {
          name: 'remote', status: 'connected', agentCount: 1, capabilities: [],
        }],
        topology: [...connected.topology, {
          id: 'remote/reviewer', name: 'reviewer', hub: 'remote', hubPath: ['remote'],
          children: [], lifecycle: 'running',
        }],
      }}
      onSelectAgent={onSelectAgent} onManage={onManage}
    />)
    fireEvent.click(screen.getByRole('button', { name: /remote.*connected/i }))
    expect(screen.queryByText('desktop-ui/main')).not.toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: /remote\/reviewer/ }))
    expect(onSelectAgent).toHaveBeenCalledWith('remote/reviewer')
    fireEvent.click(screen.getByRole('button', { name: 'Manage MetaHub' }))
    expect(onManage).toHaveBeenCalledOnce()
  })
})
