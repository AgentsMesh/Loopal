import { type DesktopHostClient } from '../backend/loopal-backend-types'

export interface SessionRuntimeWorkspace {
  readonly workspaceId: string
  readonly cwd: string
}

export interface SessionRuntimeResumeInput extends SessionRuntimeWorkspace {
  readonly sessionId: string
}

export interface SessionRuntimeHostInput {
  readonly workspaceId: string
  readonly cwd: string
  readonly resumeSessionId?: string
}

export interface SessionRuntimeAllocation {
  readonly runtimeId: string
  readonly generation: number
}
export type SessionRuntimeActivation = (sessionId: string) => Promise<void>

export type SessionRuntimeHostFactory = (
  input: SessionRuntimeHostInput,
  allocation: SessionRuntimeAllocation,
) => DesktopHostClient

export interface SessionRuntimeScope {
  readonly workspaceId: string
  readonly sessionId: string
  readonly runtimeId: string
  readonly generation: number
}

export interface SessionRuntimeHandle extends SessionRuntimeScope {
  readonly host: DesktopHostClient
}

export type SessionRuntimeHostStatus = DesktopHostClient['currentStatus']

export interface SessionRuntimeStatusEvent extends SessionRuntimeScope {
  readonly status: SessionRuntimeHostStatus
}

export interface SessionRuntimeNotificationEvent extends SessionRuntimeScope {
  readonly method: string
  readonly params: unknown
}

export interface SessionRuntimeRegistryOptions {
  readonly maxLive: number
  readonly maxTombstones?: number
  readonly createHost: SessionRuntimeHostFactory
  readonly createRuntimeId?: () => string
}
