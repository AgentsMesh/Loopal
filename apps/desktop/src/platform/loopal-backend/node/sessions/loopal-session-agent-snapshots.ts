import { type MetaHubRuntimeState } from '../../../../shared/contracts'
import { type DesktopHostClient } from '../backend/loopal-backend-types'
import { type AttentionEventKind } from '../attention/loopal-attention'
import {
  AgentListSchema,
  ViewSnapshotSchema,
  type Topology,
  type ViewSnapshot,
} from '../runtime/loopal-wire'

export interface SnapshotAttention {
  readonly kind: Extract<AttentionEventKind,
    'permission_requested' | 'question_requested' | 'plan_approval_requested'>
  readonly agentId: string
  readonly value: unknown
}

export async function loadControllableAgents(
  host: DesktopHostClient,
): Promise<ReadonlySet<string>> {
  try {
    const list = AgentListSchema.parse(await host.request('hub/list_agents', {}))
    return new Set(list.agents.filter((agent) => (
      agent.state === 'local' || agent.state === 'connected'
    )).map((agent) => agent.name))
  } catch {
    return new Set()
  }
}

export async function loadAgentSnapshots(
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

export function snapshotAttention(
  agentId: string,
  snapshot: ViewSnapshot,
): SnapshotAttention[] {
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

export function remoteAgentIds(metaHub: MetaHubRuntimeState): Set<string> {
  return new Set(metaHub.topology
    .filter((agent) => agent.hub !== metaHub.hubName)
    .map((agent) => agent.id))
}

export function remoteSnapshotAuthority(
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
