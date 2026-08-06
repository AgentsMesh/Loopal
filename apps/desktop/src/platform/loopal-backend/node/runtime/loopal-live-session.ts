import { randomUUID } from 'node:crypto'
import {
  type ConversationEntry,
  type DesktopEvent,
  type DesktopImageAttachment,
  type SessionDetail,
  type SessionStatus,
  type SessionSummary,
} from '../../../../shared/contracts'
import { LoopalEventProjector } from '../projections/loopal-event-projector'
import { projectModifiedFiles } from '../projections/loopal-artifact-projection'
import { LoopalLiveAttention } from '../attention/loopal-live-attention'
import { loadSessionDetail } from '../sessions/loopal-session-snapshot'
import { LoopalMetaHubWatcher } from '../federation/loopal-metahub-watcher'
import { type SessionRuntimeHandle } from './session-runtime-registry'
interface LiveSessionSink {
  event(event: DesktopEvent): void
  summary(summary: SessionSummary): void
}

export class LoopalLiveSession {
  private readonly projector: LoopalEventProjector
  private readonly attention: LoopalLiveAttention
  private detailValue?: SessionDetail
  private refreshing: Promise<void> | undefined
  private refreshPending = false
  private refreshShouldEmit = false
  private refreshTimer: ReturnType<typeof setTimeout> | undefined
  private readonly metaHub: LoopalMetaHubWatcher
  private active = true

  constructor(
    readonly runtime: SessionRuntimeHandle,
    private summaryValue: SessionSummary,
    private readonly now: () => Date,
    private readonly sink: LiveSessionSink,
  ) {
    this.attention = new LoopalLiveAttention(runtime, now, sink.event)
    this.projector = new LoopalEventProjector(now, {
      append: (entry) => this.append(entry),
      appendAgent: (entry, agentId) => this.appendToAgent(agentId, entry),
      updateSession: (status, attention) => this.update(status, attention),
      overflow: () => this.requestAuthoritativeRefresh(),
      attention: (kind, value, agentId) => this.attention.accept(kind, value, agentId),
      artifacts: (paths, agentId) => this.recordArtifacts(paths, agentId),
    })
    this.metaHub = new LoopalMetaHubWatcher(
      runtime.host, now, () => this.detailValue?.metaHub, () => this.refresh(true),
    )
  }

  get detail(): SessionDetail {
    if (!this.detailValue) throw new Error(`Session detail is not ready: ${this.summaryValue.id}`)
    return this.detailValue
  }

  get cachedDetail(): SessionDetail | undefined { return this.detailValue }

  replaceSummary(summary: SessionSummary): void {
    this.summaryValue = summary
    if (this.detailValue) this.detailValue = { ...this.detailValue, session: summary }
  }

  async initialize(): Promise<void> {
    await this.refresh(false)
    this.metaHub.start()
  }

  resync(): Promise<void> {
    return this.refresh(true)
  }

  accept(method: string, params: unknown): void {
    if (!this.active) return
    if (method === 'agent/event') {
      this.projector.accept(params)
      this.invalidate()
    }
    else if (method === 'view/resync_required') {
      void this.refresh(true).catch(() => undefined)
    }
  }

  async send(
    text: string, agentId = 'main', images: readonly DesktopImageAttachment[] = [],
  ): Promise<void> {
    const id = randomUUID()
    const entry: ConversationEntry = {
      id, role: 'user', text, agentId, createdAt: this.now().toISOString(),
      ...(images.length ? { imageCount: images.length } : {}),
    }
    await this.runtime.host.request('hub/route', {
      id, source: 'Human', target: { hub: [], agent: agentId },
      content: {
        text, images: images.map(({ mediaType, data }) => ({ media_type: mediaType, data })),
      },
      timestamp: this.now().toISOString(),
    })
    if (agentId === 'main') this.append(entry)
    else this.appendToAgent(agentId, entry)
    this.update('running')
  }

  dispose(): void {
    this.active = false
    if (this.refreshTimer) clearTimeout(this.refreshTimer)
    this.refreshTimer = undefined
    this.metaHub.dispose()
  }

  retire(): readonly DesktopEvent[] {
    this.active = false
    if (this.refreshTimer) clearTimeout(this.refreshTimer)
    this.refreshTimer = undefined
    this.metaHub.dispose()
    return this.attention.retire()
  }

  private refresh(emit: boolean): Promise<void> {
    this.refreshPending = true
    this.refreshShouldEmit ||= emit
    this.refreshing ??= this.runRefreshLoop().finally(() => {
      this.refreshing = undefined
      if (this.refreshPending && this.active) void this.refresh(false).catch(() => undefined)
    })
    return this.refreshing
  }
  private invalidate(): void {
    if (this.refreshTimer || !this.active) return
    this.refreshTimer = setTimeout(() => {
      this.refreshTimer = undefined
      void this.refresh(true).catch(() => undefined)
    }, 16)
  }
  private requestAuthoritativeRefresh(): void {
    if (this.refreshTimer) clearTimeout(this.refreshTimer)
    this.refreshTimer = undefined
    void this.refresh(true).catch(() => undefined)
  }
  private recordArtifacts(paths: readonly string[], agentId: string): void {
    const detail = this.detail
    const known = new Set(detail.artifacts.map((artifact) => artifact.id))
    const created = projectModifiedFiles(
      this.summaryValue.id, agentId, paths, this.now().toISOString(),
    ).filter((artifact) => !known.has(artifact.id))
    if (created.length === 0) return
    this.detailValue = {
      ...detail,
      artifacts: [...detail.artifacts, ...created],
    }
    for (const artifact of created) this.sink.event({ type: 'artifact_created', artifact })
  }

  private async runRefreshLoop(): Promise<void> {
    while (this.refreshPending && this.active) {
      this.refreshPending = false
      const emit = this.refreshShouldEmit
      this.refreshShouldEmit = false
      const snapshot = await loadSessionDetail(
        this.runtime.host, this.summaryValue, this.now, this.projector,
        this.detailValue, this.attention.remoteAgentIds(),
      )
      if (!this.active) return
      this.detailValue = { ...snapshot.detail, session: this.summaryValue }
      this.attention.reconcile(
        snapshot.pendingAttention,
        new Set(snapshot.authoritativeRemoteAgents),
      )
      this.projector.finishSync(snapshot.revision, snapshot.revisions)
      if (emit) this.sink.event({ type: 'session_detail_replaced', detail: this.detailValue })
    }
  }

  private append(entry: ConversationEntry): void {
    if (!this.active || !this.detailValue) return
    this.detailValue = {
      ...this.detailValue,
      conversation: [...this.detailValue.conversation, entry],
    }
    this.sink.event({ type: 'conversation_entry', sessionId: this.summaryValue.id, entry })
  }

  private appendToAgent(agentId: string, entry: ConversationEntry): void {
    if (!this.active || !this.detailValue) return
    const agents = this.detailValue.agents.map((agent) => agent.id === agentId
      ? { ...agent, conversation: [...agent.conversation ?? [], entry] }
      : agent)
    this.detailValue = { ...this.detailValue, agents }
    this.sink.event({ type: 'session_detail_replaced', detail: this.detailValue })
  }

  private update(status: SessionStatus, attention?: SessionSummary['attention']): void {
    if (!this.active) return
    const { attention: _old, ...summary } = this.summaryValue
    this.replaceSummary({
      ...summary,
      status,
      updatedAt: this.now().toISOString(),
      ...(attention ? { attention } : {}),
    })
    this.sink.summary(this.summaryValue)
  }
}
