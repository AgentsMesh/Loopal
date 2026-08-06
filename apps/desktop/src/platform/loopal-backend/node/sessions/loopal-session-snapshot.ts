import {
  type AgentSummary,
  type SessionDetail,
  type SessionSummary,
} from '../../../../shared/contracts'
import { type DesktopHostClient } from '../backend/loopal-backend-types'
import { LoopalEventProjector } from '../projections/loopal-event-projector'
import { projectMessages } from '../projections/loopal-message-projection'
import {
  projectAgent,
  projectSessionView,
  retiredAgent,
  topologyAgent,
} from '../projections/loopal-view-projection'
import {
  TopologySchema,
  ViewSnapshotSchema,
  type Topology,
  type ViewSnapshot,
} from '../runtime/loopal-wire'
import { mergeRemoteAgents, readMetaHubState } from '../federation/loopal-metahub-projection'
import {
  loadAgentSnapshots,
  loadControllableAgents,
  remoteAgentIds,
  remoteSnapshotAuthority,
  snapshotAttention,
  type SnapshotAttention,
} from './loopal-session-agent-snapshots'

export { type SnapshotAttention } from './loopal-session-agent-snapshots'

export async function loadSessionDetail(
  host: DesktopHostClient,
  session: SessionSummary,
  now: () => Date,
  projector: LoopalEventProjector,
  previous?: SessionDetail,
  knownRemoteAgents: ReadonlySet<string> = new Set(),
): Promise<{
  detail: SessionDetail
  revision: number
  revisions: Readonly<Record<string, number>>
  authoritativeRemoteAgents: readonly string[]
  pendingAttention: readonly SnapshotAttention[]
}> {
  projector.beginSync()
  const [mainValue, topologyValue, controllable] = await Promise.all([
    host.request('view/snapshot', { agent: 'main' }),
    host.request('hub/topology', {}),
    loadControllableAgents(host),
  ])
  const metaHub = readMetaHubState(host, now())
  const main = ViewSnapshotSchema.parse(mainValue)
  const topology = TopologySchema.parse(topologyValue)
  const currentRemoteAgents = remoteAgentIds(metaHub)
  const previousRemoteAgents = new Set([
    ...knownRemoteAgents,
    ...(previous?.agents.flatMap((agent) => (
      agent.qualifiedName ? [agent.qualifiedName] : []
    )) ?? []),
  ])
  const snapshots = await loadAgentSnapshots(
    host, main, topology, currentRemoteAgents, previousRemoteAgents,
  )
  const previousLocal = previous?.agents.filter((agent) => !agent.qualifiedName) ?? []
  const agents = mergeRemoteAgents(
    mergeAgents(snapshots, topology, previousLocal, now(), controllable),
    metaHub,
  )
  return {
    detail: {
      session,
      conversation: projectMessages(main, now(), previous?.conversation),
      agents,
      artifacts: previous?.artifacts ?? [],
      view: projectSessionView(main),
      metaHub,
    },
    revision: main.rev,
    revisions: Object.fromEntries(
      [...snapshots].map(([agentId, snapshot]) => [agentId, snapshot.rev]),
    ),
    authoritativeRemoteAgents: remoteSnapshotAuthority(
      metaHub, currentRemoteAgents, previousRemoteAgents, snapshots,
    ),
    pendingAttention: [...snapshots].flatMap(([agentId, snapshot]) => (
      snapshotAttention(agentId, snapshot)
    )),
  }
}

function mergeAgents(
  snapshots: ReadonlyMap<string, ViewSnapshot>,
  topology: Topology,
  previous: readonly AgentSummary[],
  now: Date,
  controllable: ReadonlySet<string>,
): AgentSummary[] {
  const old = new Map(previous.map((agent) => [agent.id, agent]))
  const active = new Set<string>()
  const agents: AgentSummary[] = []
  const main = snapshots.get('main')!
  agents.push(withControl(projectAgent(
    main, topology.agents.find((agent) => agent.name === 'main'), now, old.get('main'),
  ), controllable.has('main')))
  active.add('main')
  for (const entry of topology.agents) {
    if (entry.name === 'main') continue
    const snapshot = snapshots.get(entry.name)
    agents.push(withControl(snapshot
      ? projectAgent(snapshot, entry, now, old.get(entry.name))
      : topologyAgent(entry, old.get(entry.name)), controllable.has(entry.name)))
    active.add(entry.name)
  }
  for (const agent of previous) {
    if (!active.has(agent.id)) agents.push(withControl(retiredAgent(agent), false))
  }
  return agents
}

function withControl(agent: AgentSummary, controllable: boolean): AgentSummary {
  return { ...agent, controllable }
}
