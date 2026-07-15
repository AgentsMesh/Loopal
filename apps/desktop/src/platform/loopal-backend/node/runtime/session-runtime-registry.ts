import { randomUUID } from 'node:crypto'
import { Emitter, type Event } from '../../../../base/common/event'
import { SessionRuntimeActivationGate } from './session-runtime-activation'
import { SessionRuntimeEntry } from './session-runtime-registry-entry'
import {
  type SessionRuntimeHandle,
  type SessionRuntimeActivation,
  type SessionRuntimeNotificationEvent,
  type SessionRuntimeRegistryOptions,
  type SessionRuntimeResumeInput,
  type SessionRuntimeStatusEvent,
  type SessionRuntimeWorkspace,
} from './session-runtime-registry-types'
import { requirePositive, requireText } from './session-runtime-registry-validation'
export * from './session-runtime-registry-types'
interface RuntimeTombstone {
  readonly runtimeId: string
  readonly generation: number
  readonly resume: SessionRuntimeResumeInput
}
export class SessionRuntimeRegistry {
  private readonly statuses = new Emitter<SessionRuntimeStatusEvent>()
  private readonly notifications = new Emitter<SessionRuntimeNotificationEvent>()
  private readonly entries = new Map<string, SessionRuntimeEntry>()
  private readonly sessions = new Map<string, SessionRuntimeEntry>()
  private readonly tombstones = new Map<string, RuntimeTombstone>()
  private readonly createRuntimeId: () => string
  private readonly maxTombstones: number
  private generation = 0
  private closed = false
  private shutdown?: Promise<void>
  readonly onStatus: Event<SessionRuntimeStatusEvent> = this.statuses.event
  readonly onNotification: Event<SessionRuntimeNotificationEvent> = this.notifications.event

  constructor(private readonly options: SessionRuntimeRegistryOptions) {
    requirePositive(options.maxLive, 'maxLive')
    if (options.maxTombstones !== undefined) {
      requirePositive(options.maxTombstones, 'maxTombstones')
    }
    this.maxTombstones = options.maxTombstones ?? Math.max(16, options.maxLive * 4)
    this.createRuntimeId = options.createRuntimeId ?? randomUUID
  }

  get liveCount(): number {
    return this.entries.size
  }
  has(runtimeId: string): boolean { return this.entries.has(runtimeId) }
  startFresh(
    input: SessionRuntimeWorkspace,
    activate?: SessionRuntimeActivation,
  ): Promise<SessionRuntimeHandle> {
    return this.allocate(input, undefined, activate)
  }

  resume(input: SessionRuntimeResumeInput): Promise<SessionRuntimeHandle> {
    requireText(input.sessionId, 'sessionId')
    const existing = this.sessions.get(input.sessionId)
    if (existing?.retiring) return existing.retiring.then(() => this.resume(input))
    if (existing) return existing.ready!
    return this.allocate(input, input.sessionId)
  }

  get(runtimeId: string): SessionRuntimeHandle | undefined {
    return this.entries.get(runtimeId)?.handle
  }

  getBySession(sessionId: string): SessionRuntimeHandle | undefined {
    return this.sessions.get(sessionId)?.handle
  }

  listLive(): readonly SessionRuntimeHandle[] {
    return [...this.entries.values()].flatMap((entry) => entry.handle ? [entry.handle] : [])
  }

  stop(runtimeId: string): Promise<void> {
    const entry = this.entries.get(runtimeId)
    if (entry) return this.retire(entry)
    if (this.tombstones.has(runtimeId)) return Promise.resolve()
    return Promise.reject(new Error(`Unknown session runtime: ${runtimeId}`))
  }

  async restart(runtimeId: string): Promise<SessionRuntimeHandle> {
    const entry = this.entries.get(runtimeId)
    const resume = entry?.resumeInput() ?? this.tombstones.get(runtimeId)?.resume
    if (!resume) throw new Error(`Session runtime cannot be restarted: ${runtimeId}`)
    if (entry) await this.retire(entry)
    return this.resume(resume)
  }

  shutdownAll(): Promise<void> {
    this.closed = true
    this.shutdown ??= this.shutdownInternal()
    return this.shutdown
  }

  dispose(): void {
    void this.shutdownAll().catch(() => undefined)
    this.statuses.dispose()
    this.notifications.dispose()
  }

  private allocate(
    input: SessionRuntimeWorkspace,
    sessionId?: string,
    activate?: SessionRuntimeActivation,
  ): Promise<SessionRuntimeHandle> {
    if (this.closed) throw new Error('Session runtime registry is shut down')
    requireText(input.workspaceId, 'workspaceId')
    requireText(input.cwd, 'cwd')
    if (this.entries.size >= this.options.maxLive) {
      throw new Error(`Session runtime quota exceeded (${this.options.maxLive})`)
    }
    const runtimeId = this.createRuntimeId()
    if (this.entries.has(runtimeId) || this.tombstones.has(runtimeId)) {
      throw new Error(`Duplicate runtime ID: ${runtimeId}`)
    }
    const generation = ++this.generation
    let entry!: SessionRuntimeEntry
    entry = new SessionRuntimeEntry(
      input.workspaceId,
      input.cwd,
      runtimeId,
      generation,
      this.options.createHost(sessionId === undefined
        ? { workspaceId: input.workspaceId, cwd: input.cwd }
        : { workspaceId: input.workspaceId, cwd: input.cwd, resumeSessionId: sessionId },
      { runtimeId, generation }),
      sessionId,
      {
        status: (event) => this.statuses.fire(event),
        notification: (event) => this.notifications.fire(event),
        crashed: () => void this.retire(entry).catch(() => undefined),
      },
    )
    this.entries.set(runtimeId, entry)
    if (sessionId) this.sessions.set(sessionId, entry)
    entry.ready = this.startEntry(entry, sessionId, activate)
    return entry.ready
  }

  private async startEntry(
    entry: SessionRuntimeEntry,
    expected?: string,
    activate?: SessionRuntimeActivation,
  ): Promise<SessionRuntimeHandle> {
    try {
      const activation = expected ? undefined : new SessionRuntimeActivationGate(entry, activate)
      const started = await entry.host.start(activation?.activate)
      await activation?.activate(started)
      if (expected && started.sessionId !== expected) {
        throw new Error(`Desktop Host resumed ${started.sessionId}; expected ${expected}`)
      }
      const owner = this.sessions.get(started.sessionId)
      if (owner && owner !== entry) {
        throw new Error(`Session already has a live runtime: ${started.sessionId}`)
      }
      const handle = entry.bindSession(started.sessionId)
      this.sessions.set(started.sessionId, entry)
      return handle
    } catch (error) {
      await this.retire(entry)
      throw error
    }
  }

  private retire(entry: SessionRuntimeEntry): Promise<void> {
    entry.retiring ??= this.retireEntry(entry)
    return entry.retiring
  }

  private async retireEntry(entry: SessionRuntimeEntry): Promise<void> {
    try {
      await entry.host.stop()
    } finally {
      this.entries.delete(entry.runtimeId)
      const resume = entry.resumeInput()
      if (resume && this.sessions.get(resume.sessionId) === entry) {
        this.sessions.delete(resume.sessionId)
        this.remember({ runtimeId: entry.runtimeId, generation: entry.generation, resume })
      }
      entry.dispose()
      entry.host.dispose()
    }
  }

  private remember(tombstone: RuntimeTombstone): void {
    this.tombstones.set(tombstone.runtimeId, tombstone)
    while (this.tombstones.size > this.maxTombstones) {
      this.tombstones.delete(this.tombstones.keys().next().value!)
    }
  }

  private async shutdownInternal(): Promise<void> {
    const results = await Promise.allSettled([...this.entries.values()].map((entry) => {
      return this.retire(entry)
    }))
    const errors = results.flatMap((result) => result.status === 'rejected' ? [result.reason] : [])
    if (errors.length > 0) throw new AggregateError(errors, 'Failed to stop session runtimes')
  }
}
