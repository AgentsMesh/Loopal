import {
  type AgentSummary, type HostStatus, type SessionDetail, type SessionView,
} from '../../../../shared/contracts'

export type SessionPanelId =
  | 'agents' | 'tasks' | 'background' | 'scheduled'
  | 'artifacts' | 'mcp' | 'diagnostics'

export interface SessionPanelEntry {
  readonly id: SessionPanelId
  readonly label: string
  readonly count: number
  readonly alert?: boolean
}

export interface SessionPanelState {
  readonly selected?: AgentSummary
  readonly view?: SessionView
  readonly localAgents: readonly AgentSummary[]
  readonly panels: readonly SessionPanelEntry[]
}

export function buildSessionPanelState(input: {
  readonly detail?: SessionDetail
  readonly hostStatus: HostStatus
  readonly selectedAgentId: string
  readonly showTopology: boolean
}): SessionPanelState {
  const agents = input.detail?.agents ?? []
  const localAgents = agents.filter((agent) => !agent.qualifiedName)
  const selected = agents.find((agent) => agent.id === input.selectedAgentId)
    ?? agents.find((agent) => !agent.parentId) ?? agents[0]
  const view = selected?.view ?? (!selected?.parentId ? input.detail?.view : undefined)
  const panels: SessionPanelEntry[] = []
  if (input.showTopology && localAgents.length > 1) {
    panels.push({ id: 'agents', label: 'Agents', count: localAgents.length - 1 })
  }
  const activeGoal = view?.goal && view.goal.status !== 'complete'
  const activeTasks = view?.tasks.filter((task) => task.status !== 'completed') ?? []
  if (activeGoal || activeTasks.length > 0) {
    panels.push({
      id: 'tasks', label: 'Tasks', count: activeTasks.length + Number(Boolean(activeGoal)),
    })
  }
  const visibleBackground = view?.backgroundTasks.filter(
    (task) => task.status === 'running',
  ) ?? []
  if (visibleBackground.length > 0) {
    panels.push({ id: 'background', label: 'Background', count: visibleBackground.length })
  }
  if (view?.crons.length) {
    panels.push({ id: 'scheduled', label: 'Scheduled', count: view.crons.length })
  }
  if (input.detail?.artifacts.length) {
    panels.push({ id: 'artifacts', label: 'Artifacts', count: input.detail.artifacts.length })
  }
  if (view?.mcpServers.length) {
    panels.push({
      id: 'mcp', label: 'MCP', count: view.mcpServers.length,
      alert: view.mcpServers.some(isUnhealthyMcp),
    })
  }
  const diagnostics = diagnosticCount(
    input.hostStatus, selected, view,
    input.detail?.session.status === 'failed' || input.detail?.session.attention === 'failure',
  )
  if (diagnostics > 0) {
    panels.push({ id: 'diagnostics', label: 'Diagnostics', count: diagnostics, alert: true })
  }
  return { ...(selected ? { selected } : {}), ...(view ? { view } : {}), localAgents, panels }
}

function diagnosticCount(
  hostStatus: HostStatus, selected: AgentSummary | undefined, view: SessionView | undefined,
  sessionFailed: boolean,
): number {
  return Number(hostStatus === 'crashed')
    + Number(selected?.status === 'failed' || Boolean(selected?.error))
    + Number(sessionFailed)
    + Number(Boolean(view?.hubDegradedSince))
    + Number(Boolean(view?.historyTruncated))
}

function isUnhealthyMcp(server: SessionView['mcpServers'][number]): boolean {
  return server.errors.length > 0
    || !['ready', 'connected', 'running'].includes(server.status.toLocaleLowerCase())
}
