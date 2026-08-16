import { type AgentSummary, type SessionDetail, type SessionView } from '../../../../shared/contracts'
import {
  richDetail, richTimestamp, richView,
} from '../../../../../test/fixtures/workbench/rich-session'
import { buildSessionPanelState } from './session-panel-state'

describe('buildSessionPanelState', () => {
  it('does not create an idle deck for a root-only controllable session', () => {
    expect(state(idleDetail()).panels).toEqual([])
    expect(state(idleDetail(), { showTopology: false }).panels).toEqual([])
  })

  it('shows only meaningful topology and unfinished live work', () => {
    const view = {
      ...emptyView(),
      goal: goal('complete'),
      tasks: [task('done', 'completed'), task('next', 'pending')],
      backgroundTasks: [background('done', 'completed'), background('live', 'running')],
      crons: [{
        id: 'cron', schedule: '* * * * *', prompt: 'Check', recurring: true, durable: true,
      }],
    }
    const detail = idleDetail({
      agents: [root(), child(), { ...child(), id: 'remote/main', qualifiedName: 'remote/main' }],
      artifacts: [artifact()], view,
    })
    expect(state(detail).localAgents.map((agent) => agent.id)).toEqual(['agent-root', 'child'])
    expect(entries(state(detail))).toEqual([
      ['agents', 1, false], ['tasks', 1, false], ['background', 1, false],
      ['scheduled', 1, false], ['artifacts', 1, false],
    ])

    expect(entries(state({ ...detail, view: {
      ...view, goal: goal('active'), tasks: [...view.tasks, task('working', 'in_progress')],
    } }))).toContainEqual(['tasks', 3, false])
  })

  it('removes runtime work after both the goal and plan settle', () => {
    const detail = idleDetail({ view: {
      ...emptyView(), goal: goal('complete'), tasks: [task('done', 'completed')],
    } })
    expect(state(detail).panels.some((panel) => panel.id === 'tasks')).toBe(false)
  })

  it('exposes active and recent workflows as visible session work', () => {
    const run = richView().workflows.active[0] ?? {
      id: 'wrun_ui', runGoal: 'Render workflow', state: 'running' as const,
      revision: 1, outputNode: 'done', createdAt: richTimestamp, updatedAt: richTimestamp,
      counts: {
        pending: 1, ready: 0, active: 1, succeeded: 0,
        failed: 0, cancelled: 0, skipped: 0,
      },
    }
    const detail = idleDetail({ view: {
      ...emptyView(), workflows: { active: [run], recent: [{ ...run, id: 'wrun_done',
        state: 'succeeded', revision: 2 }] },
    } })
    expect(entries(state(detail))).toEqual([['workflows', 2, false]])
  })

  it('separates MCP health from contextual diagnostics', () => {
    const ready = mcp('ready', [])
    const failed = mcp('failed', ['connection lost'])
    expect(entries(state(idleDetail({ view: { ...emptyView(), mcpServers: [ready] } }))))
      .toEqual([['mcp', 1, false]])
    expect(entries(state(idleDetail({ view: { ...emptyView(), mcpServers: [ready, failed] } }))))
      .toEqual([['mcp', 2, true]])

    expect(entries(state(idleDetail(), { hostStatus: 'crashed' })))
      .toEqual([['diagnostics', 1, true]])
    expect(entries(state(idleDetail({ agents: [{ ...root(), error: 'failed' }] }))))
      .toEqual([['diagnostics', 1, true]])
    expect(entries(state(idleDetail({ view: {
      ...emptyView(), historyTruncated: true, hubDegradedSince: richTimestamp,
    } })))).toEqual([['diagnostics', 2, true]])
  })

  it('keeps MetaHub topology out of the Conversation context dock', () => {
    const disconnected = {
      state: 'disconnected' as const, hubs: [], topology: [], refreshedAt: richTimestamp,
    }
    expect(state(idleDetail({ metaHub: disconnected })).panels).toEqual([])
    const connected = {
      ...disconnected, state: 'connected' as const, address: '127.0.0.1:9900', hubName: 'local',
      hubs: [{ name: 'remote', status: 'connected' as const, agentCount: 2, capabilities: [] }],
    }
    expect(state(idleDetail({ metaHub: connected })).panels).toEqual([])
    expect(state(idleDetail({ metaHub: {
      ...connected, state: 'error', error: 'registration failed',
    } })).panels).toEqual([])
  })
})

function state(
  detail: SessionDetail,
  overrides: Partial<Parameters<typeof buildSessionPanelState>[0]> = {},
) {
  return buildSessionPanelState({
    detail, hostStatus: 'ready', selectedAgentId: 'agent-root', showTopology: true,
    ...overrides,
  })
}

function entries(value: ReturnType<typeof buildSessionPanelState>) {
  return value.panels.map((panel) => [panel.id, panel.count, panel.alert ?? false])
}

function idleDetail(overrides: Partial<SessionDetail> = {}): SessionDetail {
  return richDetail({
    agents: [root()], artifacts: [], view: emptyView(), ...overrides,
  })
}

function emptyView(): SessionView {
  const { goal: _goal, hubDegradedSince: _degraded, ...view } = richView()
  return {
    ...view, historyTruncated: false, tasks: [], backgroundTasks: [], crons: [], mcpServers: [],
  }
}

function root(): AgentSummary {
  return { id: 'agent-root', name: 'Loopal', status: 'waiting' }
}

function child(): AgentSummary {
  return { id: 'child', name: 'Child', status: 'running', parentId: 'agent-root' }
}

function task(id: string, status: 'pending' | 'in_progress' | 'completed') {
  return { id, subject: id, description: '', status, blockedBy: [], blocks: [] }
}

function background(id: string, status: 'running' | 'completed') {
  return { id, description: id, status, exitCode: null, output: '', createdAt: richTimestamp }
}

function goal(status: 'active' | 'complete') {
  return { id: 'goal', objective: 'Ship', status, createdAt: richTimestamp, updatedAt: richTimestamp }
}

function mcp(status: string, errors: string[]) {
  return {
    name: status, transport: 'stdio', source: 'workspace', status,
    toolCount: 1, resourceCount: 0, promptCount: 0, errors,
  }
}

function artifact() {
  return {
    id: 'artifact', sessionId: 'session-rich', title: 'result.md', kind: 'report' as const,
    uri: 'loopal-artifact://result', mediaType: 'text/markdown',
    producerAgentId: 'agent-root', createdAt: richTimestamp,
  }
}
