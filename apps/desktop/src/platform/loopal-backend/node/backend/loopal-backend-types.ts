import { type Event } from '../../../../base/common/event'
import { type IDisposable } from '../../../../base/common/lifecycle'
import { type HostStatus } from '../../../../shared/contracts'
import {
  type DesktopHostOptions,
  type DesktopHostActivation,
  type DesktopHostSession,
  type MetaHubStartupOptions,
} from '../../../desktop-host/node/host/desktop-host'
import { type JsonRpcNotification } from '../../../desktop-host/node/rpc/jsonrpc-client'
import {
  type SessionRuntimeHostFactory,
  type SessionRuntimeRegistry,
} from '../runtime/session-runtime-registry'
import { type SessionDirectoryRequest } from '../sessions/session-directory-authority'

export interface LoopalDesktopBackendOptions extends DesktopHostOptions {
  readonly now?: () => Date
  readonly createHost?: SessionRuntimeHostFactory
  readonly runtimeRegistry?: SessionRuntimeRegistry
  readonly maxLiveRuntimes?: number
  readonly sessionStatePath?: string
  readonly metaHubSettingsPath?: string
  readonly desktopPreferencesPath?: string
  readonly getMetaHubStartup?: () => MetaHubStartupOptions | undefined
  readonly sessionDirectoryRequest?: SessionDirectoryRequest
}

export interface DesktopHostClient extends IDisposable {
  readonly currentStatus: HostStatus
  readonly onStatus: Event<HostStatus>
  readonly onNotification: Event<JsonRpcNotification>
  start(activate?: DesktopHostActivation): Promise<DesktopHostSession>
  request(method: string, params?: unknown, signal?: AbortSignal): Promise<unknown>
  stop(): Promise<void>
}
