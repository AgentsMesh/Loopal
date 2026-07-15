import { type MetaHubRuntimeTarget } from '../../../../shared/contracts'
import { LoopalMetaHubService, type MetaHubOperations } from './loopal-metahub-service'
import { type LoopalMetaHubSettings } from './loopal-metahub-settings'
import { type LoopalSessionDirectory } from '../sessions/loopal-session-directory'
import { type SessionRuntimeHandle } from '../runtime/session-runtime-registry'

export function bindMetaHub(
  settings: LoopalMetaHubSettings,
  directory: LoopalSessionDirectory,
  now: () => Date,
): MetaHubOperations {
  const service = new LoopalMetaHubService({
    settings,
    runtime: (target) => resolveRuntime(directory, target),
    resync: async (sessionId) => {
      const live = directory.liveSession(sessionId)
      if (live) await live.resync()
    },
    now,
  })
  return {
    getMetaHubSettings: service.getMetaHubSettings.bind(service),
    updateMetaHubSettings: service.updateMetaHubSettings.bind(service),
    getMetaHubStatus: service.getMetaHubStatus.bind(service),
    joinMetaHub: service.joinMetaHub.bind(service),
    disconnectMetaHub: service.disconnectMetaHub.bind(service),
  }
}

function resolveRuntime(
  directory: LoopalSessionDirectory,
  target: MetaHubRuntimeTarget,
): SessionRuntimeHandle {
  const runtime = directory.runtimeForSession(target.sessionId)
  if (!runtime || runtime.runtimeId !== target.runtimeId
    || runtime.generation !== target.generation || runtime.host.currentStatus !== 'ready') {
    throw Object.assign(new Error(`Session runtime is gone: ${target.sessionId}`), {
      code: 'RUNTIME_GONE',
    })
  }
  return runtime
}
