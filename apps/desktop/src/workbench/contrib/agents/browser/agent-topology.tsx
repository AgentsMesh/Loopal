import { useState } from 'react'
import { type AgentSummary } from '../../../../shared/contracts'
import { useI18n } from '../../../browser/i18n-context'

interface AgentTopologyProps {
  readonly agents: readonly AgentSummary[]
  readonly selectedAgentId?: string | undefined
  readonly onSelect?: ((agentId: string) => void) | undefined
}

export function AgentTopology(props: AgentTopologyProps): React.JSX.Element {
  const { t } = useI18n()
  const [showAll, setShowAll] = useState(false)
  const retained = props.agents.filter((agent) => isRetained(agent))
  const activeCount = props.agents.length - retained.length
  const visible = showAll ? props.agents : props.agents.filter((agent) => (
    !isRetained(agent) || !agent.parentId || agent.id === props.selectedAgentId
  ))
  const graph = buildGraph(visible)
  return (
    <div
      className="inspector-content agent-topology"
      data-testid="agents-pane"
      aria-label={t('topology.label')}
    >
      {retained.length > 0 && (
        <div className="agent-topology-toolbar">
          <span>{t('topology.counts', { active: activeCount, retained: retained.length })}</span>
          <div role="group" aria-label={t('topology.filter')}>
            <button aria-pressed={!showAll} onClick={() => setShowAll(false)}>{t('topology.active')}</button>
            <button aria-pressed={showAll} onClick={() => setShowAll(true)}>{t('topology.all')}</button>
          </div>
        </div>
      )}
      <div className="agent-topology-canvas" role="tree" onKeyDown={moveTreeFocus}>
        {graph.roots.map((agent) => (
          <TopologyBranch
            key={agent.id} agent={agent} graph={graph}
            selectedAgentId={props.selectedAgentId} onSelect={props.onSelect}
            path={new Set()} level={1}
          />
        ))}
      </div>
      {props.agents.length === 0 && (
        <div className="empty-inspector"><span>◎</span><p>{t('topology.empty')}</p></div>
      )}
    </div>
  )
}

interface AgentGraph {
  readonly byId: ReadonlyMap<string, AgentSummary>
  readonly children: ReadonlyMap<string, readonly AgentSummary[]>
  readonly roots: readonly AgentSummary[]
}

function TopologyBranch(props: {
  readonly agent: AgentSummary
  readonly graph: AgentGraph
  readonly selectedAgentId?: string | undefined
  readonly onSelect?: ((agentId: string) => void) | undefined
  readonly path: ReadonlySet<string>
  readonly level: number
}): React.JSX.Element {
  const { t } = useI18n()
  const nextPath = new Set(props.path).add(props.agent.id)
  const children = (props.graph.children.get(props.agent.id) ?? [])
    .filter((child) => !nextPath.has(child.id))
  const parent = props.agent.parentId ? props.graph.byId.get(props.agent.parentId) : undefined
  return (
    <div className="topology-branch">
      <button
        role="treeitem" aria-level={props.level}
        aria-selected={props.selectedAgentId === props.agent.id}
        className={`topology-node ${props.selectedAgentId === props.agent.id ? 'selected' : ''}`}
        data-agent-id={props.agent.id} data-parent-id={props.agent.parentId}
        onClick={() => props.onSelect?.(props.agent.id)}
      >
        <span className={`agent-state agent-${props.agent.status}`} />
        <span className="topology-node-copy">
          <strong>{props.agent.name}</strong>
          <small>{props.agent.status}{props.agent.controllable === false
            ? ` · ${t(isRetained(props.agent) ? 'topology.retained' : 'topology.unavailable')}`
            : ''}</small>
          <small>{parent
            ? t('topology.childOf', { name: parent.name })
            : t('topology.root')}{runtimeLabel(props.agent)}</small>
          {props.agent.error && <small className="diagnostic-error">{props.agent.error}</small>}
        </span>
      </button>
      {children.length > 0 && (
        <div className="topology-children" role="group">
          {children.map((child) => (
            <TopologyBranch
              key={child.id} agent={child} graph={props.graph}
              selectedAgentId={props.selectedAgentId} onSelect={props.onSelect}
              path={nextPath} level={props.level + 1}
            />
          ))}
        </div>
      )}
    </div>
  )
}

function buildGraph(agents: readonly AgentSummary[]): AgentGraph {
  const byId = new Map(agents.map((agent) => [agent.id, agent]))
  const children = new Map<string, AgentSummary[]>()
  const claimed = new Set<string>()
  const attach = (parentId: string, child: AgentSummary): void => {
    const list = children.get(parentId) ?? []
    if (!list.some((candidate) => candidate.id === child.id)) list.push(child)
    children.set(parentId, list)
    claimed.add(child.id)
  }
  for (const agent of agents) {
    if (agent.parentId && byId.has(agent.parentId)) attach(agent.parentId, agent)
  }
  for (const parent of agents) {
    for (const childId of parent.children ?? []) {
      const child = byId.get(childId)
      if (child) attach(parent.id, child)
    }
  }
  const roots = agents.filter((agent) => !claimed.has(agent.id))
  const reached = new Set<string>()
  const visit = (agent: AgentSummary): void => {
    if (reached.has(agent.id)) return
    reached.add(agent.id)
    for (const child of children.get(agent.id) ?? []) visit(child)
  }
  roots.forEach(visit)
  for (const agent of agents) {
    if (!reached.has(agent.id)) {
      roots.push(agent)
      visit(agent)
    }
  }
  return { byId, children, roots }
}

function runtimeLabel(agent: AgentSummary): string {
  const model = agent.model ? ` · ${agent.model}` : ''
  const tool = agent.lastTool ? ` · ${agent.lastTool}` : ''
  return `${model}${tool}`
}

function isRetained(agent: AgentSummary): boolean {
  return agent.status === 'completed' || agent.status === 'failed'
}

function moveTreeFocus(event: React.KeyboardEvent<HTMLDivElement>): void {
  if (!['ArrowUp', 'ArrowDown', 'Home', 'End'].includes(event.key)) return
  if (!(event.target instanceof HTMLButtonElement)) return
  const nodes = Array.from(event.currentTarget.querySelectorAll<HTMLButtonElement>('[role="treeitem"]'))
  const current = nodes.indexOf(event.target)
  const next = event.key === 'Home' ? 0 : event.key === 'End' ? nodes.length - 1
    : Math.max(0, Math.min(nodes.length - 1, current + (event.key === 'ArrowDown' ? 1 : -1)))
  event.preventDefault()
  nodes[next]?.focus()
}
