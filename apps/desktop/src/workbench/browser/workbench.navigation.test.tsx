import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import {
  createTestAPI,
  sessionDetail,
  sessionOne,
  sessionTwo,
  updatedAt,
} from '../../../test/support/workbench/api-stub'
import { Workbench } from './workbench'

describe('Workbench navigation', () => {
  it('loads, filters, switches sessions, and navigates inspector panes', async () => {
    const { api } = createTestAPI()
    render(<Workbench api={api} />)

    expect(await screen.findByTestId('active-session-title')).toHaveTextContent(
      'Build the desktop workbench',
    )
    expect(screen.queryByTestId('host-status')).not.toBeInTheDocument()
    expect(screen.queryByTestId('session-panel-zone')).not.toBeInTheDocument()
    expect(screen.queryByTestId('inspector')).not.toBeInTheDocument()

    const search = screen.getByLabelText('Search sessions')
    const sessionList = screen.getByTestId('session-list')
    fireEvent.change(search, { target: { value: 'protocol' } })
    expect(within(sessionList).queryByText('Build the desktop workbench')).not.toBeInTheDocument()
    expect(within(sessionList).getByText('Version the protocol')).toBeInTheDocument()
    fireEvent.change(search, { target: { value: '' } })

    fireEvent.click(screen.getByText('Version the protocol'))
    await waitFor(() => {
      expect(screen.getByTestId('active-session-title')).toHaveTextContent('Version the protocol')
    })
    expect(screen.getByText('Protocol.md')).toBeInTheDocument()

    expect(screen.getByRole('tab', { name: 'Artifacts' })).toHaveAttribute(
      'aria-selected', 'true',
    )
    expect(screen.getByTestId('artifacts-pane')).toHaveTextContent('Protocol.md')
    expect(screen.queryByRole('tab', { name: 'Diagnostics' })).not.toBeInTheDocument()

    fireEvent.keyDown(window, { key: 'k', metaKey: true })
    expect(search).toHaveFocus()
    search.blur()
    fireEvent.keyDown(window, { key: 'K', ctrlKey: true })
    expect(search).toHaveFocus()
    fireEvent.keyDown(window, { key: 'x' })
  })

  it('renders an empty session catalog safely', async () => {
    const { api } = createTestAPI({
      bootstrap: async () => ({
        protocolVersion: 2,
        hostStatus: 'stopped',
        workspaces: [],
        sessions: [],
        runtimes: [],
      }),
    })
    render(<Workbench api={api} />)

    await waitFor(() => expect(screen.getByTestId('active-session-title'))
      .toHaveTextContent('Select a session'))
    expect(screen.getByTestId('active-session-title')).toHaveTextContent('Select a session')
    expect(within(screen.getByTestId('session-list')).queryAllByRole('button')).toHaveLength(0)
    fireEvent.keyDown(screen.getByLabelText('Message Loopal'), { key: 'Enter' })
  })

  it('renders Federation as an application workspace without session chrome', async () => {
    const { api } = createTestAPI()
    render(<Workbench api={api} />)
    await screen.findByTestId('active-session-title')
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Federation' }))
      await Promise.resolve()
    })
    const workspace = screen.getByTestId('primary-workspace')
    expect(workspace).toHaveAttribute('data-workspace', 'federation')
    expect(within(workspace).queryByTestId('active-session-title')).not.toBeInTheDocument()
    expect(within(workspace).queryByText('Workspace', { exact: true })).not.toBeInTheDocument()
    expect(within(workspace).queryByText('Session', { exact: true })).not.toBeInTheDocument()
    expect(within(workspace).queryByRole('button', { name: 'Stop session' }))
      .not.toBeInTheDocument()
    expect(within(workspace).queryByRole('button', { name: 'Restart session' }))
      .not.toBeInTheDocument()
    expect(screen.getByRole('heading', {
      name: 'Start a Federation for your Loopal sessions.',
    })).toBeInTheDocument()
  })

  it('opens a non-active Federation owner and keeps its qualified Agent selected', async () => {
    const address = '127.0.0.1:39000'
    const topology = [
      { id: 'one/main', name: 'main', hub: 'one', hubPath: ['one'], children: [],
        lifecycle: 'running' as const },
      { id: 'two/reviewer', name: 'reviewer', hub: 'two', hubPath: ['two'],
        parentId: 'two/main', children: [], lifecycle: 'running' as const },
    ]
    const state = (hubName: string) => ({
      state: 'connected' as const, address, hubName,
      hubs: ['one', 'two'].map((name) => ({
        name, status: 'connected' as const, agentCount: 1, capabilities: [],
      })), topology, refreshedAt: updatedAt,
    })
    const openSession = vi.fn(async (sessionId: string) => sessionId === sessionTwo.id
      ? { ...sessionDetail(sessionTwo), metaHub: state('two'), agents: [{
          id: 'agent-session-2', name: 'Loopal', status: 'waiting' as const,
        }, {
          id: 'shadow-reviewer', name: 'reviewer', status: 'running' as const,
          parentId: 'agent-session-2', qualifiedName: 'two/reviewer', conversation: [],
        }] }
      : { ...sessionDetail(sessionOne), metaHub: state('one') })
    const { api } = createTestAPI({
      openSession,
      getLocalMetaHubStatus: async () => ({ state: 'running', address }),
      getMetaHubSettings: async () => ({
        address, hubName: 'desktop', joinOnStart: false,
        startLocalOnLaunch: true, tokenConfigured: true,
      }),
      getMetaHubStatus: async (target) => state(
        target.sessionId === sessionTwo.id ? 'two' : 'one',
      ),
    })
    render(<Workbench api={api} />)
    await screen.findByTestId('active-session-title')
    fireEvent.click(screen.getByRole('button', { name: 'Federation' }))
    await screen.findByTestId('federation-connection')
    const card = screen.getByTestId('federation-agent-list').querySelector<HTMLElement>(
      `[data-owner-session-id="${sessionTwo.id}"][data-agent-id="two/reviewer"]`,
    )
    expect(card).not.toBeNull()
    fireEvent.click(card!)
    fireEvent.click(screen.getByRole('button', { name: 'Open conversation' }))
    await waitFor(() => expect(screen.getByTestId('primary-workspace'))
      .toHaveAttribute('data-workspace', 'conversation'))
    expect(screen.getByTestId('active-session-title')).toHaveTextContent(sessionTwo.title)
    expect(screen.getByText(/Viewing reviewer/)).toBeInTheDocument()
    expect(openSession).toHaveBeenLastCalledWith(sessionTwo.id)
  })

  it('does not let a stale session request replace the latest selection', async () => {
    let initial = true
    let resolveOne: (() => void) | undefined
    let resolveTwo: (() => void) | undefined
    const one = { ...sessionDetail(sessionOne), conversation: [{
      ...sessionDetail(sessionOne).conversation[0]!, text: 'Latest selection',
    }] }
    const two = { ...sessionDetail(sessionTwo), conversation: [{
      ...sessionDetail(sessionTwo).conversation[0]!, text: 'Stale selection',
    }] }
    const openSession = vi.fn((sessionId: string) => {
      if (initial) {
        initial = false
        return Promise.resolve(sessionDetail(sessionOne))
      }
      return new Promise<typeof one>((resolve) => {
        const finish = (): void => resolve(sessionId === sessionOne.id ? one : two)
        if (sessionId === sessionOne.id) resolveOne = finish
        else resolveTwo = finish
      })
    })
    const { api } = createTestAPI({ openSession })
    render(<Workbench api={api} />)
    await screen.findByText('Conversation for Build the desktop workbench')

    const list = within(screen.getByTestId('session-list'))
    fireEvent.click(list.getByText('Version the protocol'))
    fireEvent.click(list.getByText('Build the desktop workbench'))
    await waitFor(() => {
      expect(resolveOne).toBeTypeOf('function')
      expect(resolveTwo).toBeTypeOf('function')
    })
    await act(async () => resolveOne?.())
    expect(await screen.findByText('Latest selection')).toBeInTheDocument()
    await act(async () => resolveTwo?.())
    expect(screen.queryByText('Stale selection')).not.toBeInTheDocument()
  })
})
