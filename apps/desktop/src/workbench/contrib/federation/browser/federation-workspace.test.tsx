import { fireEvent, render, screen, within } from '@testing-library/react'
import { type SessionDetail } from '../../../../shared/contracts'
import { richDetail, richTimestamp } from '../../../../../test/fixtures/workbench/rich-session'
import { aggregateFederation } from './federation-model'
import { FederationWorkspace } from './federation-workspace'

describe('FederationWorkspace', () => {
  it('starts and manages an application-level federation while stopped', () => {
    const onManage = vi.fn()
    const onStart = vi.fn(async () => undefined)
    render(<FederationWorkspace snapshot={aggregateFederation(
      { state: 'stopped' }, {}, [],
    )}
      onStart={onStart} onRefresh={vi.fn()} onOpenConversation={vi.fn()}
      onManage={onManage} />)
    expect(screen.getByTestId('federation-workspace')).toHaveClass('federation-empty')
    expect(screen.getByRole('heading', {
      name: 'Start a Federation for your Loopal sessions.',
    })).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'Start Federation' }))
    expect(onStart).toHaveBeenCalledOnce()
    fireEvent.click(screen.getByRole('button', { name: 'Manage federation' }))
    expect(onManage).toHaveBeenCalledOnce()
  })

  it('shows a running coordinator without binding the empty state to a session', () => {
    const onRefresh = vi.fn(async () => undefined)
    render(<FederationWorkspace snapshot={aggregateFederation(
      { state: 'running', address: '127.0.0.1:39000' }, {}, [],
    )} onStart={vi.fn()} onRefresh={onRefresh}
      onOpenConversation={vi.fn()} onManage={vi.fn()} />)
    expect(screen.getByRole('heading', { name: 'Federation is running.' })).toBeInTheDocument()
    expect(screen.getByText(/Right-click a session/)).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'Refresh' }))
    expect(onRefresh).toHaveBeenCalledOnce()
  })

  it('filters hubs and opens a topology node through its owner session', () => {
    const onOpenConversation = vi.fn()
    const detail = connectedDetail()
    const target = { sessionId: detail.session.id, runtimeId: 'runtime-rich', generation: 1 }
    const remoteTarget = { sessionId: 'remote-session', runtimeId: 'runtime-remote', generation: 1 }
    render(<FederationWorkspace snapshot={aggregateFederation(
      { state: 'running', address: '127.0.0.1:39000' },
      { [detail.session.id]: target, [remoteTarget.sessionId]: remoteTarget },
      [{ target, state: detail.metaHub! }, {
        target: remoteTarget, state: { ...detail.metaHub!, hubName: 'remote' },
      }],
    )} onStart={vi.fn()}
      onRefresh={vi.fn()} onOpenConversation={onOpenConversation} onManage={vi.fn()} />)
    const workspace = screen.getByTestId('federation-workspace')
    expect(workspace).not.toHaveClass('navigator-hidden')
    expect(screen.getByTestId('federation-connection')).toHaveTextContent(
      '2 hubs · 3 Agents · 2 sessions joined',
    )
    expect(screen.getByTestId('federation-connection')).toHaveTextContent('Updated')
    expect(agentCard('local/root')).toBeInTheDocument()

    fireEvent.click(hubCard('remote'))
    expect(agentCard('remote/reviewer')).toBeInTheDocument()
    expect(agentCard('remote/planner')).toBeInTheDocument()
    expect(queryAgentCard('local/root')).not.toBeInTheDocument()
    expect(screen.getByTestId('federation-agent-list')).toHaveTextContent('2 Agents')

    const open = screen.getByRole('button', { name: 'Open conversation' })
    expect(open).toBeEnabled()
    fireEvent.click(open)
    expect(onOpenConversation).toHaveBeenCalledWith({
      sessionId: 'remote-session', agentId: 'remote/reviewer',
    })

    fireEvent.click(agentCard('remote/planner'))
    expect(screen.getByTestId('federation-agent-detail')).toHaveTextContent('Planner')
    expect(open).toBeEnabled()
    fireEvent.click(open)
    expect(onOpenConversation).toHaveBeenLastCalledWith({
      sessionId: 'remote-session', agentId: 'remote/planner',
    })

    fireEvent.click(within(screen.getByTestId('federation-hub-list'))
      .getByRole('button', { name: /All hubs/ }))
    expect(agentCard('local/root')).toBeInTheDocument()
  })

  it('does not open a remote Hub without a local owner session', () => {
    const detail = connectedDetail()
    const target = { sessionId: detail.session.id, runtimeId: 'runtime-rich', generation: 1 }
    const state = { ...detail.metaHub!, topology: detail.metaHub!.topology
      .filter(({ hub }) => hub === 'remote') }
    render(<FederationWorkspace snapshot={aggregateFederation(
      { state: 'running', address: '127.0.0.1:39000' },
      { [detail.session.id]: target }, [{ target, state }],
    )} onStart={vi.fn()} onRefresh={vi.fn()} onOpenConversation={vi.fn()}
      onManage={vi.fn()} />)
    expect(screen.getByRole('button', { name: 'Open conversation' })).toBeDisabled()
    expect(screen.getByText('This topology node has no projected conversation yet.'))
      .toBeInTheDocument()
  })
})

function connectedDetail(): SessionDetail {
  return richDetail({
    agents: [
      { id: 'root', name: 'Loopal', status: 'waiting' },
      { id: 'shadow-reviewer', name: 'Reviewer', status: 'running',
        qualifiedName: 'remote/reviewer', shadow: true },
    ],
    metaHub: {
      state: 'connected', address: '127.0.0.1:39000', hubName: 'local',
      hubs: [
        { name: 'local', status: 'connected', agentCount: 1, capabilities: ['chat'] },
        { name: 'remote', status: 'connected', agentCount: 2, capabilities: ['review'] },
      ],
      topology: [
        topologyAgent('local/root', 'Loopal', 'local'),
        topologyAgent('remote/reviewer', 'Reviewer', 'remote'),
        topologyAgent('remote/planner', 'Planner', 'remote'),
      ],
      refreshedAt: richTimestamp,
    },
  })
}

function topologyAgent(id: string, name: string, hub: string) {
  return { id, name, hub, hubPath: [hub], children: [], lifecycle: 'running' as const }
}

function hubCard(id: string): HTMLElement {
  const card = screen.getByTestId('federation-hub-list')
    .querySelector<HTMLElement>(`[data-hub-id="${id}"]`)
  if (!card) throw new Error(`Missing hub ${id}`)
  return card
}

function agentCard(id: string): HTMLElement {
  const card = queryAgentCard(id)
  if (!card) throw new Error(`Missing agent ${id}`)
  return card
}

function queryAgentCard(id: string): HTMLElement | null {
  return screen.getByTestId('federation-agent-list')
    .querySelector<HTMLElement>(`[data-agent-id="${id}"]`)
}
