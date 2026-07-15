import { type Event } from '../../../../base/common/event'
import { type HostStatus } from '../../../../shared/contracts'
import { type DesktopHostClient } from '../backend/loopal-backend-types'
import {
  type DesktopHostActivation, type DesktopHostSession,
} from '../../../desktop-host/node/host/desktop-host'
import { type JsonRpcNotification } from '../../../desktop-host/node/rpc/jsonrpc-client'

const HOST_WORKSPACE_ID = 'local-workspace'

export class WorkspaceScopedHost implements DesktopHostClient {
  readonly onStatus: Event<HostStatus>
  readonly onNotification: Event<JsonRpcNotification>

  constructor(
    private readonly host: DesktopHostClient,
    private readonly workspaceId: string,
  ) {
    this.onStatus = host.onStatus
    this.onNotification = (listener) => host.onNotification((event) => listener({
      ...event, params: remap(event.params, HOST_WORKSPACE_ID, workspaceId),
    }))
  }

  get currentStatus(): HostStatus { return this.host.currentStatus }
  start(activate?: DesktopHostActivation): Promise<DesktopHostSession> {
    return this.host.start(activate)
  }
  stop(): Promise<void> { return this.host.stop() }
  dispose(): void { this.host.dispose() }

  async request(method: string, params?: unknown, signal?: AbortSignal): Promise<unknown> {
    const value = await this.host.request(
      method, remap(params, this.workspaceId, HOST_WORKSPACE_ID), signal,
    )
    return remap(value, HOST_WORKSPACE_ID, this.workspaceId)
  }
}

function remap(value: unknown, from: string, to: string): unknown {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return value
  const record = value as Record<string, unknown>
  return record.workspaceId === from ? { ...record, workspaceId: to } : value
}
