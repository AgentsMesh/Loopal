import { DisposableStore } from '../../../../base/common/lifecycle'
import { type DesktopHostClient } from '../backend/loopal-backend-types'
import {
  type SessionRuntimeHandle,
  type SessionRuntimeHostStatus,
  type SessionRuntimeNotificationEvent,
  type SessionRuntimeStatusEvent,
  type SessionRuntimeWorkspace,
} from './session-runtime-registry-types'

const PRE_READY_BUFFER_LIMIT = 64

interface EntrySink {
  status(event: SessionRuntimeStatusEvent): void
  notification(event: SessionRuntimeNotificationEvent): void
  crashed(): void
}

export class SessionRuntimeEntry implements SessionRuntimeWorkspace {
  readonly subscriptions = new DisposableStore()
  readonly statuses: SessionRuntimeHostStatus[] = []
  readonly notifications: Array<{ method: string; params: unknown }> = []
  sessionId: string | undefined
  handle?: SessionRuntimeHandle
  ready?: Promise<SessionRuntimeHandle>
  retiring?: Promise<void>
  private overflowed = false
  private createdSessionId?: string

  constructor(
    readonly workspaceId: string,
    readonly cwd: string,
    readonly runtimeId: string,
    readonly generation: number,
    readonly host: DesktopHostClient,
    sessionId: string | undefined,
    private readonly sink: EntrySink,
  ) {
    this.sessionId = sessionId
    this.subscriptions.add(host.onStatus((status) => this.acceptStatus(status)))
    this.subscriptions.add(host.onNotification((event) => {
      this.acceptNotification(event.method, event.params)
    }))
  }

  bindSession(sessionId: string): SessionRuntimeHandle {
    this.markSessionCreated(sessionId)
    this.sessionId = sessionId
    this.handle = Object.freeze({
      ...this.scope(),
      host: this.host,
    })
    for (const status of this.statuses.splice(0)) {
      this.sink.status({ ...this.scope(), status })
    }
    if (this.overflowed) {
      this.sink.notification({
        ...this.scope(),
        method: 'view/resync_required',
        params: { reason: 'pre_ready_buffer_overflow' },
      })
    } else {
      for (const event of this.notifications.splice(0)) {
        this.sink.notification({ ...this.scope(), ...event })
      }
    }
    this.notifications.length = 0
    return this.handle
  }

  markSessionCreated(sessionId: string): void {
    const known = this.sessionId ?? this.createdSessionId
    if (known && known !== sessionId) {
      throw new Error(`Desktop Host changed Session from ${known} to ${sessionId}`)
    }
    this.createdSessionId = sessionId
  }

  resumeInput() {
    const sessionId = this.sessionId ?? this.createdSessionId
    if (!sessionId) return undefined
    return {
      workspaceId: this.workspaceId,
      cwd: this.cwd,
      sessionId,
    }
  }

  dispose(): void {
    this.subscriptions.dispose()
  }

  private acceptStatus(status: SessionRuntimeHostStatus): void {
    if (this.sessionId) this.sink.status({ ...this.scope(), status })
    else this.bufferStatus(status)
    if (status === 'crashed') this.sink.crashed()
  }

  private acceptNotification(method: string, params: unknown): void {
    if (this.sessionId) {
      this.sink.notification({ ...this.scope(), method, params })
    } else if (!this.overflowed) {
      if (this.bufferSize() >= PRE_READY_BUFFER_LIMIT) this.markOverflow()
      else this.notifications.push({ method, params })
    }
  }

  private bufferStatus(status: SessionRuntimeHostStatus): void {
    if (this.overflowed) {
      this.statuses.splice(0, this.statuses.length, status)
    } else if (this.bufferSize() >= PRE_READY_BUFFER_LIMIT) {
      this.markOverflow()
      this.statuses.push(status)
    } else {
      this.statuses.push(status)
    }
  }

  private markOverflow(): void {
    this.overflowed = true
    this.statuses.length = 0
    this.notifications.length = 0
  }

  private bufferSize(): number {
    return this.statuses.length + this.notifications.length
  }

  private scope() {
    return {
      workspaceId: this.workspaceId,
      sessionId: this.sessionId!,
      runtimeId: this.runtimeId,
      generation: this.generation,
    }
  }
}
