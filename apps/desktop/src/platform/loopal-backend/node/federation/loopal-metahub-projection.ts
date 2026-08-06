import {
  type AgentSummary,
  type MetaHubRuntimeState,
  type MetaHubTopologyAgent,
} from '../../../../shared/contracts'
import {
  HubStatusWireSchema,
  MetaHubListWireSchema,
  MetaHubTopologyWireSchema,
  type MetaHubTopologyWire,
} from './loopal-metahub-wire'
import { type DesktopHostClient } from '../backend/loopal-backend-types'

const CACHE_TTL_MS = 2_000
const QUERY_TIMEOUT_MS = 2_500
interface CachedState {
  value?: MetaHubRuntimeState
  loadedAt?: number
  pending?: Promise<MetaHubRuntimeState>
}
const states = new WeakMap<DesktopHostClient, CachedState>()

export async function loadMetaHubState(
  host: DesktopHostClient,
  now: Date,
  force = false,
): Promise<MetaHubRuntimeState> {
  const cache = states.get(host) ?? {}
  states.set(host, cache)
  if (!force && cache.value && cache.loadedAt !== undefined
    && now.getTime() - cache.loadedAt < CACHE_TTL_MS) return cache.value
  if (cache.pending) return cache.pending
  cache.pending = queryMetaHubState(host, now).then((value) => {
    cache.value = value
    cache.loadedAt = now.getTime()
    return value
  }).finally(() => { delete cache.pending })
  return cache.pending
}

export function invalidateMetaHubState(host: DesktopHostClient): void {
  states.delete(host)
}

export function readMetaHubState(
  host: DesktopHostClient,
  now: Date,
): MetaHubRuntimeState {
  return states.get(host)?.value ?? {
    state: 'disconnected', hubs: [], topology: [], refreshedAt: now.toISOString(),
  }
}

export function sameMetaHubState(
  left: MetaHubRuntimeState | undefined,
  right: MetaHubRuntimeState,
): boolean {
  if (!left) return false
  const { refreshedAt: _leftTime, ...leftValue } = left
  const { refreshedAt: _rightTime, ...rightValue } = right
  return JSON.stringify(leftValue) === JSON.stringify(rightValue)
}

async function queryMetaHubState(
  host: DesktopHostClient,
  now: Date,
): Promise<MetaHubRuntimeState> {
  let status: ReturnType<typeof HubStatusWireSchema.parse>
  try {
    status = HubStatusWireSchema.parse(await request(host, 'hub/status'))
  } catch (error) {
    return {
      state: 'error', hubs: [], topology: [], error: errorMessage(error),
      refreshedAt: now.toISOString(),
    }
  }
  if (!status.uplink?.connected) {
    return { state: 'disconnected', hubs: [], topology: [], refreshedAt: now.toISOString() }
  }
  try {
    const [list, topology] = await Promise.all([
      request(host, 'meta/list_hubs'),
      request(host, 'meta/topology'),
    ])
    const topologyValue = MetaHubTopologyWireSchema.parse(topology)
    const topologyUnavailableHubs = topologyValue.hubs
      .filter((entry) => 'error' in entry.topology)
      .map((entry) => entry.hub)
      .sort((left, right) => left.localeCompare(right))
    return {
      state: 'connected',
      address: status.uplink.address ?? undefined,
      hubName: status.uplink.hub_name,
      hubs: MetaHubListWireSchema.parse(list).hubs.map((hub) => ({
        name: hub.name,
        status: hubStatus(hub.status),
        agentCount: hub.agent_count,
        capabilities: hub.capabilities,
      })).sort((left, right) => left.name.localeCompare(right.name)),
      topology: projectTopology(topologyValue),
      ...(topologyUnavailableHubs.length > 0 ? { topologyUnavailableHubs } : {}),
      refreshedAt: now.toISOString(),
    }
  } catch (error) {
    return {
      state: 'error',
      address: status.uplink.address ?? undefined,
      hubName: status.uplink.hub_name,
      hubs: [], topology: [], error: errorMessage(error), refreshedAt: now.toISOString(),
    }
  }
}

async function request(host: DesktopHostClient, method: string): Promise<unknown> {
  const controller = new AbortController()
  const timer = setTimeout(() => controller.abort(), QUERY_TIMEOUT_MS)
  try { return await host.request(method, {}, controller.signal) }
  finally { clearTimeout(timer) }
}

export function projectTopology(value: MetaHubTopologyWire): MetaHubTopologyAgent[] {
  return value.hubs.flatMap(({ hub, topology }) => {
    if ('error' in topology) return []
    return topology.agents.map((agent) => {
      const id = qualified(hub, agent.name)
      return {
        id, name: agent.name, hub, hubPath: [hub], lifecycle: agent.lifecycle,
        children: agent.children.map((child) => qualified(hub, child)),
        ...(agent.parent ? { parentId: parentId(hub, agent.parent) } : {}),
        ...(agent.model ? { model: agent.model } : {}),
        ...(agent.error ? { error: agent.error } : {}),
      }
    })
  }).sort((left, right) => left.id.localeCompare(right.id))
}

export function mergeRemoteAgents(
  local: readonly AgentSummary[],
  state: MetaHubRuntimeState,
): AgentSummary[] {
  const localHub = state.hubName
  const remoteTopology = state.topology.filter((agent) => agent.hub !== localHub)
  const remote = remoteTopology.map((agent) => ({
    id: agent.id,
    name: `${agent.name} · ${agent.hub}`,
    status: lifecycleStatus(agent.lifecycle),
    parentId: normalizeLocal(agent.parentId, localHub),
    children: agent.children.map((child) => normalizeLocal(child, localHub)!),
    model: agent.model,
    error: agent.error,
    hubPath: agent.hubPath,
    qualifiedName: agent.id,
    controllable: agent.lifecycle === 'running' || agent.lifecycle === 'spawning',
  })).map(removeUndefined)
  const replacedProxies = new Set(local.filter((agent) => agent.shadow
    && remoteTopology.some((remoteAgent) => remoteAgent.name === agent.id
      && normalizeLocal(remoteAgent.parentId, localHub) === agent.parentId))
    .map((agent) => agent.id))
  const retained = local.filter((agent) => !replacedProxies.has(agent.id)).map((agent) => ({
    ...agent,
    ...(agent.children ? {
      children: agent.children.filter((child) => !replacedProxies.has(child)),
    } : {}),
  }))
  const ids = new Set(retained.map((agent) => agent.id))
  return [...retained, ...remote.filter((agent) => !ids.has(agent.id))]
}

function qualified(hub: string, agent: string): string { return `${hub}/${leaf(agent)}` }
function normalizeLocal(value: string | undefined, localHub: string | undefined): string | undefined {
  if (!value || !localHub || !value.startsWith(`${localHub}/`)) return value
  return leaf(value)
}
function parentId(hub: string, parent: string): string {
  return parent.includes('/') ? parent : qualified(hub, parent)
}
function leaf(value: string): string { return value.split('/').at(-1) || value }
function hubStatus(value: string): 'connected' | 'degraded' | 'disconnected' {
  const status = value.toLowerCase()
  return status === 'degraded' || status === 'disconnected' ? status : 'connected'
}
function lifecycleStatus(value: MetaHubTopologyAgent['lifecycle']): AgentSummary['status'] {
  if (value === 'spawning') return 'starting'
  if (value === 'finished') return 'completed'
  if (value === 'failed') return 'failed'
  return 'running'
}
function errorMessage(error: unknown): string {
  return (error instanceof Error ? error.message : String(error)).slice(0, 500)
}
function removeUndefined<T extends Record<string, unknown>>(value: T): T {
  return Object.fromEntries(Object.entries(value).filter(([, item]) => item !== undefined)) as T
}
