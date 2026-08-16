import { fireEvent, render, screen } from '@testing-library/react'
import {
  type AgentControlCommand, type SessionDetail, type SessionView,
} from '../../../../shared/contracts'
import { richDetail, richTimestamp, richView } from '../../../../../test/fixtures/workbench/rich-session'
import { SessionPanelZone } from './session-panel-zone'

const artifact = {
  id: 'artifact', sessionId: 'session-rich', title: 'result.md', kind: 'report' as const,
  uri: 'loopal-artifact://result', mediaType: 'text/markdown',
  producerAgentId: 'agent-root', createdAt: richTimestamp,
}

describe('SessionPanelZone', () => {
  it('starts collapsed and toggles one horizontal panel from its active tab', () => {
    const actions = mount(richPanelDetail())
    expect(screen.getByRole('tablist', { name: 'Session panels' })).toBeInTheDocument()
    expect(screen.getAllByRole('tab').map((tab) => tab.textContent)).toEqual([
      'Agents1', 'Tasks3', 'Background1', 'Scheduled1',
      'Artifacts1', 'MCP1', 'Diagnostics1',
    ])
    const agents = screen.getByRole('tab', { name: 'Agents' })
    expect(agents).toHaveAttribute('aria-selected', 'true')
    expect(agents).toHaveAttribute('aria-expanded', 'false')
    expect(visiblePanels()).toHaveLength(0)

    fireEvent.click(agents)
    expect(agents).toHaveAttribute('aria-expanded', 'true')
    expect(screen.getByTestId('agents-pane')).toBeVisible()
    expect(visiblePanels()).toHaveLength(1)
    fireEvent.click(agents)
    expect(visiblePanels()).toHaveLength(0)

    fireEvent.click(screen.getByRole('tab', { name: 'Tasks' }))
    expect(screen.getByTestId('tasks-pane')).toBeVisible()
    expect(screen.getByTestId('agents-pane')).not.toBeVisible()
    fireEvent.click(screen.getByRole('tab', { name: 'Agents' }))
    fireEvent.click(screen.getByRole('treeitem', { name: /E2E specialist/ }))
    expect(actions.onSelectAgent).toHaveBeenCalledWith('agent-e2e')
  })

  it('supports keyboard selection, explicit collapse, and escape', () => {
    mount(richPanelDetail())
    const agents = screen.getByRole('tab', { name: 'Agents' })
    fireEvent.keyDown(agents, { key: 'ArrowRight' })
    expect(screen.getByRole('tab', { name: 'Tasks' })).toHaveFocus()
    expect(screen.getByTestId('tasks-pane')).toBeVisible()
    fireEvent.keyDown(screen.getByRole('tab', { name: 'Tasks' }), { key: 'End' })
    expect(screen.getByRole('tab', { name: 'Diagnostics' })).toHaveFocus()
    fireEvent.keyDown(screen.getByRole('tab', { name: 'Diagnostics' }), { key: 'Home' })
    expect(agents).toHaveFocus()
    fireEvent.keyDown(agents, { key: 'Home' })
    expect(screen.getByTestId('agents-pane')).toBeVisible()
    fireEvent.keyDown(agents, { key: 'ArrowLeft' })
    expect(screen.getByRole('tab', { name: 'Diagnostics' })).toHaveFocus()

    expect(screen.queryByRole('separator')).not.toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'Collapse session panel' }))
    expect(visiblePanels()).toHaveLength(0)
    fireEvent.click(screen.getByRole('button', { name: 'Expand session panel' }))
    fireEvent.keyDown(screen.getByTestId('session-panel-zone'), { key: 'Escape' })
    expect(visiblePanels()).toHaveLength(0)
  })

  it('remembers active panel and collapsed state for each session', () => {
    const first = richPanelDetail('first')
    const actions = mount(first)
    fireEvent.click(screen.getByRole('tab', { name: 'Tasks' }))
    expect(screen.getByTestId('tasks-pane')).toBeVisible()

    actions.rerender(element(richPanelDetail('second'), actions))
    expect(screen.getByRole('tab', { name: 'Agents' })).toHaveAttribute('aria-selected', 'true')
    expect(visiblePanels()).toHaveLength(0)
    fireEvent.click(screen.getByRole('tab', { name: 'Agents' }))

    actions.rerender(element(first, actions))
    expect(screen.getByRole('tab', { name: 'Tasks' })).toHaveAttribute('aria-selected', 'true')
    expect(screen.getByTestId('tasks-pane')).toBeVisible()
    actions.rerender(element(richPanelDetail('second'), actions))
    expect(screen.getByTestId('agents-pane')).toBeVisible()
  })

  it('falls back when the active panel disappears', () => {
    const tasks = taskOnlyDetail()
    const actions = mount(tasks)
    fireEvent.click(screen.getByRole('tab', { name: 'Tasks' }))
    const withChild = { ...tasks, agents: [...tasks.agents, {
      id: 'child', name: 'Child', status: 'running' as const, parentId: 'agent-root',
    }] }
    actions.rerender(element(withChild, actions))
    expect(screen.getByRole('tab', { name: 'Tasks' })).toHaveAttribute('aria-selected', 'true')
    actions.rerender(element({ ...withChild, view: emptyView() }, actions))
    expect(screen.getByRole('tab', { name: 'Agents' })).toHaveAttribute('aria-selected', 'true')
    actions.rerender(element({ ...tasks, view: emptyView() }, actions))
    expect(screen.queryByTestId('session-panel-zone')).not.toBeInTheDocument()
  })

  it('routes projected workflows through the session panel deck', () => {
    const run = {
      id: 'wrun_zone', runGoal: 'Render production workflow', state: 'running' as const,
      revision: 2, outputNode: 'done', createdAt: richTimestamp, updatedAt: richTimestamp,
      counts: {
        pending: 1, ready: 0, active: 1, succeeded: 0,
        failed: 0, cancelled: 0, skipped: 0,
      },
    }
    mount(richDetail({
      agents: [{ id: 'agent-root', name: 'Loopal', status: 'waiting' }],
      artifacts: [],
      view: { ...emptyView(), workflows: { active: [run], recent: [] } },
    }))
    const tab = screen.getByRole('tab', { name: 'Workflows' })
    expect(tab).toHaveTextContent('1')
    fireEvent.click(tab)
    expect(screen.getByTestId('workflows-pane')).toBeVisible()
    expect(screen.getByText('Render production workflow')).toBeInTheDocument()
  })
})

function mount(detail: SessionDetail) {
  const actions = {
    onSelectAgent: vi.fn<(agentId: string) => void>(),
    onControl: vi.fn<(command: AgentControlCommand) => void>(),
  }
  const view = render(element(detail, actions))
  return { ...actions, rerender: view.rerender }
}

function element(detail: SessionDetail, actions: {
  onSelectAgent: (agentId: string) => void
  onControl: (command: AgentControlCommand) => void
}) {
  return <SessionPanelZone detail={detail} hostStatus="ready" selectedAgentId="agent-root"
    onSelectAgent={actions.onSelectAgent} canControl busy={false}
    onControl={actions.onControl} showTopology />
}

function richPanelDetail(sessionId = 'session-rich'): SessionDetail {
  return richDetail({ session: { ...richDetail().session, id: sessionId }, artifacts: [artifact],
    metaHub: { state: 'connected', address: '127.0.0.1:9900', hubName: 'desktop',
      hubs: [{ name: 'remote', status: 'connected', agentCount: 1, capabilities: [] }],
      topology: [{ id: 'remote/reviewer', name: 'reviewer', hub: 'remote',
        hubPath: ['remote'], children: [], lifecycle: 'running' }],
      refreshedAt: richTimestamp } })
}

function taskOnlyDetail(): SessionDetail {
  return richDetail({ agents: [{ id: 'agent-root', name: 'Loopal', status: 'waiting' }],
    artifacts: [], view: { ...emptyView(), tasks: [{ id: 'pending', subject: 'Pending',
      description: '', status: 'pending', blockedBy: [], blocks: [] }] } })
}

function emptyView(): SessionView {
  const { goal: _goal, hubDegradedSince: _degraded, ...view } = richView()
  return { ...view, historyTruncated: false, tasks: [], backgroundTasks: [], crons: [],
    mcpServers: [] }
}

function visiblePanels(): HTMLElement[] {
  return screen.getAllByRole('tabpanel', { hidden: true }).filter((panel) => !panel.hidden)
}
