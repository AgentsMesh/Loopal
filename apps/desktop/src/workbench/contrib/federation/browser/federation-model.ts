import {
  type LocalMetaHubStatus,
  type MetaHubInfo,
  type MetaHubRuntimeState,
  type MetaHubRuntimeTarget,
  type MetaHubTopologyAgent,
  type RuntimeSummary,
  type SessionSummary,
} from '../../../../shared/contracts'
import { resolveMetaHubRuntimeTarget } from '../../sessions/browser/session-runtime-target'

export interface FederationConnection {
  readonly target: MetaHubRuntimeTarget
  readonly state: MetaHubRuntimeState
}

export interface FederationTopologyNode {
  readonly sessionId?: string
  readonly agent: MetaHubTopologyAgent
}

export interface FederationConversationTarget {
  readonly sessionId: string
  readonly agentId: string
}

export type FederationMembership = MetaHubRuntimeState['state'] | 'external' | 'unavailable'

export interface FederationSnapshot {
  readonly local: LocalMetaHubStatus
  readonly network: MetaHubRuntimeState
  readonly connections: readonly FederationConnection[]
  readonly topology: readonly FederationTopologyNode[]
  readonly memberships: Readonly<Record<string, FederationMembership>>
}

export function federationTargets(
  sessions: readonly SessionSummary[],
  runtimes: readonly RuntimeSummary[],
): Readonly<Record<string, MetaHubRuntimeTarget>> {
  const targets: Record<string, MetaHubRuntimeTarget> = {}
  for (const session of sessions) {
    const target = resolveMetaHubRuntimeTarget(session.id, sessions, runtimes)
    if (target) targets[session.id] = target
  }
  return targets
}

export function aggregateFederation(
  local: LocalMetaHubStatus,
  targets: Readonly<Record<string, MetaHubRuntimeTarget>>,
  connections: readonly FederationConnection[],
  address?: string,
): FederationSnapshot {
  const currentAddress = address?.trim()
    || (local.state === 'running' ? local.address?.trim() : undefined)
  const memberships: Record<string, FederationMembership> = {}
  for (const sessionId of Object.keys(targets)) memberships[sessionId] = 'disconnected'
  for (const connection of connections) {
    memberships[connection.target.sessionId] = membership(connection.state, currentAddress)
  }
  const scoped = connections.filter(({ target }) => (
    memberships[target.sessionId] !== 'external'
  ))
  const connected = scoped.filter(({ state }) => state.state === 'connected')
  const errors = scoped.filter(({ state }) => state.state === 'error')
  const topology = ownedTopology(connected)
  const first = connected[0]?.state
  const refreshedAt = newestTimestamp(scoped)
  const network: MetaHubRuntimeState = {
    state: connected.length ? 'connected' : errors.length ? 'error' : 'disconnected',
    hubs: uniqueHubs(connected.flatMap(({ state }) => state.hubs)),
    topology: topology.map(({ agent }) => agent),
    refreshedAt,
    ...(currentAddress ? { address: currentAddress } : first?.address ? { address: first.address } : {}),
    ...(first?.hubName ? { hubName: first.hubName } : {}),
    ...(errors.length ? { error: errors.map(({ state }) => state.error)
      .filter(Boolean).join('; ') || 'Federation status unavailable' } : {}),
  }
  return { local, network, connections: scoped, topology, memberships }
}

function ownedTopology(
  connections: readonly FederationConnection[],
): FederationTopologyNode[] {
  const hubOwners = new Map(connections.flatMap(({ target, state }) =>
    state.hubName ? [[state.hubName, target.sessionId] as const] : []))
  const nodes = new Map<string, FederationTopologyNode>()
  for (const { target, state } of connections) {
    for (const agent of state.topology) {
      const owner = agent.hub === state.hubName
        ? target.sessionId : hubOwners.get(agent.hub)
      nodes.set(`${owner ?? '__external__'}\u0000${agent.id}`,
        owner ? { sessionId: owner, agent } : { agent })
    }
  }
  return [...nodes.values()]
}

function membership(state: MetaHubRuntimeState, address: string | undefined): FederationMembership {
  const stateAddress = state.address?.trim() || undefined
  if (stateAddress && stateAddress !== address) return 'external'
  if (state.state === 'connected' && (!address || !stateAddress)) return 'external'
  return state.state
}

function uniqueHubs(values: readonly MetaHubInfo[]): MetaHubInfo[] {
  const hubs = new Map<string, MetaHubInfo>()
  for (const value of values) {
    const current = hubs.get(value.name)
    hubs.set(value.name, current ? {
      ...value,
      agentCount: Math.max(current.agentCount, value.agentCount),
      capabilities: [...new Set([...current.capabilities, ...value.capabilities])],
      status: current.status === 'connected' ? current.status : value.status,
    } : value)
  }
  return [...hubs.values()]
}

function newestTimestamp(connections: readonly FederationConnection[]): string {
  return connections.map(({ state }) => state.refreshedAt).sort().at(-1)
    ?? new Date(0).toISOString()
}
