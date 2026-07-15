import { LoopalDesktopHost } from '../../../desktop-host/node/host/desktop-host'
import { federationHubName } from '../../../../shared/contracts/metahub-identity'
import { type LoopalDesktopBackendOptions } from './loopal-backend-types'
import { SessionRuntimeRegistry } from '../runtime/session-runtime-registry'
import { WorkspaceScopedHost } from '../workspace/workspace-scoped-host'

export function createBackendRegistry(
  options: LoopalDesktopBackendOptions,
): SessionRuntimeRegistry {
  if (options.runtimeRegistry) return options.runtimeRegistry
  const createHost = options.createHost ?? ((input, allocation) => {
    const startup = options.getMetaHubStartup?.()
    const metaHub = startup ? {
      ...startup,
      hubName: federationHubName(startup.hubName, {
        sessionId: input.resumeSessionId ?? allocation.runtimeId,
        runtimeId: allocation.runtimeId,
        generation: allocation.generation,
      }),
    } : undefined
    const host = new LoopalDesktopHost({
      binaryPath: options.binaryPath,
      cwd: input.cwd,
      ...(input.resumeSessionId === undefined ? {} : { resumeSessionId: input.resumeSessionId }),
      ...(options.parentPid === undefined ? {} : { parentPid: options.parentPid }),
      ...(options.env === undefined ? {} : { env: options.env }),
      ...(options.startupTimeoutMs === undefined ? {} : { startupTimeoutMs: options.startupTimeoutMs }),
      ...(options.shutdownTimeoutMs === undefined
        ? {} : { shutdownTimeoutMs: options.shutdownTimeoutMs }),
      ...(options.clientName === undefined ? {} : { clientName: options.clientName }),
      ...(options.spawnProcess === undefined ? {} : { spawnProcess: options.spawnProcess }),
      ...(options.connectRpc === undefined ? {} : { connectRpc: options.connectRpc }),
      ...(metaHub ? { metaHub } : {}),
    })
    return new WorkspaceScopedHost(host, input.workspaceId)
  })
  return new SessionRuntimeRegistry({
    maxLive: options.maxLiveRuntimes ?? 4,
    createHost,
  })
}
