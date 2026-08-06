import {
  type AgentSummary,
  type MetaHubRuntimeState,
  type SessionDetail,
  type SessionSummary,
} from '../../../../shared/contracts'
import { type DesktopHostClient } from '../backend/loopal-backend-types'
import { type AttentionEventKind } from '../attention/loopal-attention'
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
  AgentListSchema,
  type Topology,
  type ViewSnapshot,
} from '../runtime/loopal-wire'
import { mergeRemoteAgents, readMetaHubState } from '../federation/loopal-metahub-projection'

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

async function loadControllableAgents(host: DesktopHostClient): Promise<ReadonlySet<string>> {
  try {
    const list = AgentListSchema.parse(await host.request('hub/list_agents', {}))
    return new Set(list.agents.filter((agent) => (
      agent.state === 'local' || agent.state === 'connected'
    )).map((agent) => agent.name))
  } catch {
    return new Set()
  }
}

export interface SnapshotAttention {
  readonly kind: Extract<AttentionEventKind,
    'permission_requested' | 'question_requested' | 'plan_approval_requested'>
  readonly agentId: string
  readonly value: unknown
}

function snapshotAttention(agentId: string, snapshot: ViewSnapshot): SnapshotAttention[] {
  const conversation = snapshot.state.agent.conversation
  const pending: SnapshotAttention[] = []
  if (conversation.pending_permission) {
    pending.push({
      kind: 'permission_requested', agentId, value: conversation.pending_permission,
    })
  }
  if (conversation.pending_question) {
    const question = conversation.pending_question
    pending.push({
      kind: 'question_requested', agentId,
      value: {
        id: question.id,
        questions: question.questions,
        classifier_running: question.classifier_status?.kind === 'running',
        classifier_status: question.classifier_status,
      },
    })
  }
  if (conversation.pending_plan_approval) {
    pending.push({
      kind: 'plan_approval_requested', agentId,
      value: conversation.pending_plan_approval,
    })
  }
  return pending
}

async function loadAgentSnapshots(
  host: DesktopHostClient,
  main: ViewSnapshot,
  topology: Topology,
  currentRemoteAgents: ReadonlySet<string>,
  previousRemoteAgents: ReadonlySet<string>,
): Promise<Map<string, ViewSnapshot>> {
  const result = new Map<string, ViewSnapshot>([['main', main]])
  const names = new Set([
    ...topology.agents.map((agent) => agent.name).filter((name) => name !== 'main'),
    ...currentRemoteAgents,
    ...previousRemoteAgents,
  ])
  const snapshots = await Promise.all([...names].map(async (name) => {
    try {
      return [name, ViewSnapshotSchema.parse(
        await host.request('view/snapshot', { agent: name }),
      )] as const
    } catch {
      return undefined
    }
  }))
  for (const snapshot of snapshots) {
    if (snapshot) result.set(snapshot[0], snapshot[1])
  }
  return result
}

function remoteAgentIds(metaHub: MetaHubRuntimeState): Set<string> {
  return new Set(metaHub.topology
    .filter((agent) => agent.hub !== metaHub.hubName)
    .map((agent) => agent.id))
}

function remoteSnapshotAuthority(
  metaHub: MetaHubRuntimeState,
  currentRemoteAgents: ReadonlySet<string>,
  previousRemoteAgents: ReadonlySet<string>,
  snapshots: ReadonlyMap<string, ViewSnapshot>,
): string[] {
  const authoritative = new Set([...snapshots.keys()].filter((agentId) => agentId.includes('/')))
  if (metaHub.state !== 'connected') return [...authoritative]
  const unavailableHubs = new Set(metaHub.topologyUnavailableHubs ?? [])
  for (const agentId of previousRemoteAgents) {
    const hub = agentId.split('/', 1)[0]
    if (!currentRemoteAgents.has(agentId) && hub && !unavailableHubs.has(hub)) {
      authoritative.add(agentId)
    }
  }
  return [...authoritative]
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
