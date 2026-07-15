import { CancellationToken, throwIfCancelled } from '../../../../base/common/cancellation'
import {
  type LocalMetaHubStatus,
  type MetaHubRuntimeState,
  type MetaHubRuntimeTarget,
  type MetaHubSettings,
} from '../../../../shared/contracts'
import { type MetaHubOperations } from '../federation/loopal-metahub-service'

export interface FakeMetaHubOperations extends MetaHubOperations {
  getLocalMetaHubStatus(token?: CancellationToken): Promise<LocalMetaHubStatus>
  startLocalMetaHub(
    input: { bindAddress: string }, token?: CancellationToken,
  ): Promise<LocalMetaHubStatus>
  stopLocalMetaHub(token?: CancellationToken): Promise<LocalMetaHubStatus>
}

export function bindFakeMetaHub(
  hasRuntime: (target: MetaHubRuntimeTarget) => boolean,
  publish?: (target: MetaHubRuntimeTarget, state: MetaHubRuntimeState) => void,
): FakeMetaHubOperations {
  let secret: string | undefined
  let settings: MetaHubSettings = {
    address: '', hubName: 'desktop-fake', joinOnStart: false,
    startLocalOnLaunch: false, tokenConfigured: false,
  }
  const connectedTargets = new Map<string, {
    readonly target: MetaHubRuntimeTarget
    readonly address: string
    readonly hubName: string
  }>()
  let local: LocalMetaHubStatus = { state: 'stopped' }
  const disconnected = (): MetaHubRuntimeState => ({
    state: 'disconnected', hubs: [], topology: [], refreshedAt: new Date().toISOString(),
  })
  const prune = (): void => {
    for (const [key, { target }] of connectedTargets) {
      if (!hasRuntime(target)) connectedTargets.delete(key)
    }
  }
  const state = (key: string): MetaHubRuntimeState => {
    prune()
    const membership = connectedTargets.get(key)
    if (!membership) return disconnected()
    const members = [...connectedTargets.values()]
    return {
      state: 'connected', address: membership.address, hubName: membership.hubName,
      hubs: members.map(({ hubName }) => ({
        name: hubName, status: 'connected', agentCount: 1, capabilities: ['desktop'],
      })),
      topology: members.map(({ hubName }) => ({
        id: `${hubName}/main`, name: 'main', hub: hubName,
        hubPath: [hubName], children: [], lifecycle: 'running',
      })),
      refreshedAt: new Date().toISOString(),
    }
  }
  const requireRuntime = (target: MetaHubRuntimeTarget, token: CancellationToken): void => {
    throwIfCancelled(token)
    if (!hasRuntime(target)) throw new Error(`Session runtime is gone: ${target.sessionId}`)
  }
  const publishConnected = (): void => {
    prune()
    for (const [key, membership] of connectedTargets) {
      publish?.(membership.target, state(key))
    }
  }
  return {
    getMetaHubSettings: async (token = CancellationToken.None) => {
      throwIfCancelled(token); return { ...settings }
    },
    updateMetaHubSettings: async (input, token = CancellationToken.None) => {
      throwIfCancelled(token)
      secret = input.clearToken ? undefined : input.token ?? secret
      settings = { ...input, tokenConfigured: Boolean(secret) }
      delete (settings as Partial<typeof input>).token
      delete (settings as Partial<typeof input>).clearToken
      return { ...settings }
    },
    getMetaHubStatus: async (target, token = CancellationToken.None) => {
      requireRuntime(target, token); return state(targetKey(target))
    },
    joinMetaHub: async (input, token = CancellationToken.None) => {
      requireRuntime(input, token)
      if (!input.token && !secret) throw new Error('MetaHub token is required')
      const activeTarget = targetCopy(input)
      const key = targetKey(activeTarget)
      connectedTargets.set(key, {
        target: activeTarget,
        address: input.address ?? settings.address,
        hubName: input.hubName ?? settings.hubName,
      })
      const next = state(key)
      publishConnected()
      return next
    },
    disconnectMetaHub: async (target, token = CancellationToken.None) => {
      requireRuntime(target, token)
      const activeTarget = targetCopy(target)
      connectedTargets.delete(targetKey(activeTarget))
      const next = disconnected()
      publish?.(activeTarget, next)
      publishConnected()
      return next
    },
    getLocalMetaHubStatus: async (token = CancellationToken.None) => {
      throwIfCancelled(token); return { ...local }
    },
    startLocalMetaHub: async (_input, token = CancellationToken.None) => {
      throwIfCancelled(token)
      local = { state: 'running', address: '127.0.0.1:39000' }
      settings = { ...settings, address: local.address!, tokenConfigured: true }
      secret = 'fake-managed-secret'
      return { ...local }
    },
    stopLocalMetaHub: async (token = CancellationToken.None) => {
      throwIfCancelled(token)
      prune()
      const targets = [...connectedTargets.values()].map(({ target }) => target)
      connectedTargets.clear()
      local = { state: 'stopped' }
      secret = undefined
      settings = {
        ...settings, address: '', joinOnStart: false, tokenConfigured: false,
      }
      for (const activeTarget of targets) publish?.(activeTarget, disconnected())
      return { ...local }
    },
  }
}

function targetCopy(value: MetaHubRuntimeTarget): MetaHubRuntimeTarget {
  return {
    sessionId: value.sessionId, runtimeId: value.runtimeId, generation: value.generation,
  }
}

function targetKey(value: MetaHubRuntimeTarget): string {
  return JSON.stringify([value.sessionId, value.runtimeId, value.generation])
}
