import { DisposableStore } from '../../../../base/common/lifecycle'
import {
  type DesktopEvent,
  type RuntimeSummary,
  type SessionDetail,
  type SessionSummary,
} from '../../../../shared/contracts'
import { LoopalLiveSession } from '../runtime/loopal-live-session'
import {
  fallbackSession,
  stoppedSession,
  type CatalogSession,
} from './loopal-session-catalog'
import {
  type SessionRuntimeHandle,
  type SessionRuntimeNotificationEvent,
  type SessionRuntimeScope,
  type SessionRuntimeStatusEvent,
  SessionRuntimeRegistry,
} from '../runtime/session-runtime-registry'
import { LoopalWorkspaceLeaders } from '../workspace/loopal-workspace-leaders'
import {
  hostSession,
  runtimeFields,
  runtimeSummary,
} from '../projections/loopal-runtime-projection'
const PENDING_LIMIT = 64
interface DirectorySink {
  event(event: DesktopEvent): void
  service(event: SessionRuntimeNotificationEvent): void
}
export class LoopalSessionDirectory {
  private readonly subscriptions = new DisposableStore()
  private readonly sessions = new Map<string, SessionSummary>()
  private readonly runtimes = new Map<string, RuntimeSummary>()
  private readonly live = new Map<string, LoopalLiveSession>()
  private readonly details = new Map<string, SessionDetail>()
  private readonly pending = new Map<string, SessionRuntimeNotificationEvent[]>()
  readonly leaders = new LoopalWorkspaceLeaders()
  constructor(
    private readonly registry: SessionRuntimeRegistry,
    private readonly now: () => Date,
    private readonly workspaceName: string,
    private readonly sink: DirectorySink,
  ) {
    this.subscriptions.add(registry.onStatus((event) => this.acceptStatus(event)))
    this.subscriptions.add(registry.onNotification((event) => this.acceptNotification(event)))
  }
  session(id: string): SessionSummary | undefined { return this.sessions.get(id) }
  liveSession(id: string): LoopalLiveSession | undefined { return this.live.get(id) }
  detail(id: string): SessionDetail | undefined {
    const session = this.sessions.get(id)
    if (!session) return undefined
    const cached = this.live.get(id)?.cachedDetail ?? this.details.get(id)
    return cached
      ? { ...cached, session }
      : { session, conversation: [], agents: [], artifacts: [] }
  }
  runtimeForSession(id: string): SessionRuntimeHandle | undefined {
    return this.live.get(id)?.runtime
  }
  runtime(id: string): RuntimeSummary | undefined { return this.runtimes.get(id) }
  sessionValues(): readonly SessionSummary[] { return [...this.sessions.values()] }
  runtimeValues(): readonly RuntimeSummary[] { return [...this.runtimes.values()] }
  markSessionUnavailable(id: string): void {
    const session = this.sessions.get(id)
    if (!session) return
    const { activeRuntimeId: _runtime, ...stopped } = session
    this.storeSummary({ ...stopped, status: 'failed', attention: 'failure' }, false)
  }

  mergeCatalog(
    values: readonly CatalogSession[],
    workspaceId: string,
    emit = false,
  ): void {
    for (const value of values) {
      const previous = this.sessions.get(value.id)
      const ownerWorkspaceId = previous?.workspaceId ?? workspaceId
      const next = previous?.activeRuntimeId
        ? { ...stoppedSession(value, ownerWorkspaceId), ...runtimeFields(previous) }
        : stoppedSession(value, ownerWorkspaceId)
      this.storeSummary(next, emit)
    }
  }

  async attach(runtime: SessionRuntimeHandle, emit = true): Promise<LoopalLiveSession> {
    const becameLeader = this.leaders.add(runtime)
    if (becameLeader && emit) {
      this.sink.event({ type: 'host_status', status: runtime.host.currentStatus })
    }
    const currentRuntime = this.runtimes.get(runtime.runtimeId)
      ?? runtimeSummary(runtime, runtime.host.currentStatus, this.now())
    this.storeRuntime(currentRuntime)
    if (emit) this.sink.event({ type: 'runtime_updated', runtime: currentRuntime })
    const existing = this.sessions.get(runtime.sessionId)
      ?? fallbackSession(
        runtime.sessionId, runtime.workspaceId, this.workspaceName, this.now().toISOString(),
      )
    const { attention: _attention, activeRuntimeId: _oldRuntime, ...base } = existing
    const summary = {
      ...base,
      status: 'waiting' as const,
      activeRuntimeId: runtime.runtimeId,
      updatedAt: this.now().toISOString(),
    }
    this.storeSummary(summary, emit)
    const previous = this.live.get(runtime.sessionId)
    if (previous && previous.runtime.runtimeId !== runtime.runtimeId) previous.dispose()
    const state = new LoopalLiveSession(runtime, summary, this.now, {
      event: (event) => this.sink.event(event),
      summary: (next) => this.storeSummary(next, true),
    })
    this.live.set(runtime.sessionId, state)
    for (const event of this.pending.get(runtime.runtimeId) ?? []) {
      state.accept(event.method, event.params)
    }
    this.pending.delete(runtime.runtimeId)
    await state.initialize()
    return state
  }

  dispose(): void {
    this.subscriptions.dispose()
    for (const state of this.live.values()) state.dispose()
    this.live.clear()
    this.pending.clear()
  }

  private acceptStatus(event: SessionRuntimeStatusEvent): void {
    const previous = this.runtimes.get(event.runtimeId)
    if (previous?.state === 'crashed' && event.status !== 'crashed') return
    const runtime = runtimeSummary(event, event.status, this.now(), previous?.startedAt)
    if (!this.storeRuntime(runtime)) return
    this.sink.event({ type: 'runtime_updated', runtime })
    const session = this.sessions.get(event.sessionId)
    if (session) this.storeSummary(hostSession(session, event, this.now().toISOString()), true)
    const hostStatuses = this.leaders.transition(
      event.runtimeId, event.workspaceId, event.status,
    )
    if (event.status === 'stopping' || event.status === 'stopped' || event.status === 'crashed') {
      this.retireLive(event)
    }
    for (const status of hostStatuses) this.sink.event({ type: 'host_status', status })
  }

  private acceptNotification(event: SessionRuntimeNotificationEvent): void {
    if (event.method !== 'agent/event' && event.method !== 'view/resync_required') {
      this.sink.service(event)
      return
    }
    const state = this.live.get(event.sessionId)
    if (state?.runtime.runtimeId === event.runtimeId
      && state.runtime.generation === event.generation) {
      state.accept(event.method, event.params)
    } else if (!state && this.registry.has(event.runtimeId)) {
      this.buffer(event)
    }
  }

  private buffer(event: SessionRuntimeNotificationEvent): void {
    const buffered = this.pending.get(event.runtimeId) ?? []
    if (buffered.length >= PENDING_LIMIT) {
      buffered.splice(0, buffered.length, {
        ...event,
        method: 'view/resync_required',
        params: { reason: 'session_attach_buffer_overflow' },
      })
    } else if (buffered[0]?.method !== 'view/resync_required') {
      buffered.push(event)
    }
    this.pending.set(event.runtimeId, buffered)
  }

  private storeSummary(summary: SessionSummary, emit: boolean): void {
    this.sessions.set(summary.id, summary)
    this.live.get(summary.id)?.replaceSummary(summary)
    if (emit) this.sink.event({ type: 'session_updated', session: summary })
  }

  private storeRuntime(runtime: RuntimeSummary): boolean {
    for (const existing of this.runtimes.values()) {
      if (existing.sessionId !== runtime.sessionId || existing.id === runtime.id) continue
      if (existing.generation > runtime.generation) return false
      this.runtimes.delete(existing.id)
    }
    this.runtimes.set(runtime.id, runtime)
    return true
  }

  private retireLive(scope: SessionRuntimeScope): void {
    const state = this.live.get(scope.sessionId)
    if (state?.runtime.runtimeId === scope.runtimeId) {
      if (state.cachedDetail) this.details.set(scope.sessionId, state.cachedDetail)
      for (const event of state.retire()) this.sink.event(event)
      this.live.delete(scope.sessionId)
    }
    this.pending.delete(scope.runtimeId)
  }
}
