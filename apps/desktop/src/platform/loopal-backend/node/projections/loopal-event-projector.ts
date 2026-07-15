import { randomUUID } from 'node:crypto'
import {
  type ConversationEntry,
  type SessionStatus,
  type SessionSummary,
} from '../../../../shared/contracts'
import { AgentEventSchema } from '../runtime/loopal-wire'
import { attentionKindForPayload, type AttentionEventKind } from '../attention/loopal-attention'
import { projectEventNotice } from './loopal-event-notice'

export { normalizeAgentStatus, normalizeRole } from './loopal-event-normalizers'

interface ProjectionSink {
  append(entry: ConversationEntry): void
  appendAgent?(entry: ConversationEntry, agentId: string): void
  updateSession(status: SessionStatus, attention?: SessionSummary['attention']): void
  attention(kind: AttentionEventKind, value: unknown, agentId: string): void
  artifacts?(paths: readonly string[], agentId: string): void
  overflow?(): void
}

const SYNC_BUFFER_LIMIT = 64

export class LoopalEventProjector {
  private assistantBuffer = ''
  private syncing = true
  private readonly lastRevisions = new Map<string, number>()
  private readonly buffered: unknown[] = []
  private overflowed = false
  private failed = false

  constructor(
    private readonly now: () => Date,
    private readonly sink: ProjectionSink,
  ) {}

  beginSync(): void {
    this.syncing = true
    this.assistantBuffer = ''
    this.overflowed = false
  }

  finishSync(revision: number, revisions?: Readonly<Record<string, number>>): void {
    this.lastRevisions.set('main', revision)
    for (const [agentId, value] of Object.entries(revisions ?? {})) {
      this.lastRevisions.set(agentId, value)
    }
    const buffered = this.buffered.splice(0)
    this.syncing = false
    if (this.overflowed) {
      this.overflowed = false
      this.sink.overflow?.()
      return
    }
    for (const event of buffered) this.apply(event)
  }

  accept(value: unknown): void {
    if (!this.syncing) {
      this.apply(value)
      return
    }
    if (this.overflowed) return
    if (this.buffered.length >= SYNC_BUFFER_LIMIT) {
      this.buffered.length = 0
      this.overflowed = true
    } else this.buffered.push(value)
  }

  private apply(value: unknown): void {
    const event = AgentEventSchema.safeParse(value)
    if (!event.success) return
    const address = event.data.agent_name
    if (!address || address.hub.length !== 0) return
    if (event.data.rev !== undefined) {
      if (event.data.rev <= (this.lastRevisions.get(address.agent) ?? 0)) return
      this.lastRevisions.set(address.agent, event.data.rev)
    }
    const payload = unpackPayload(event.data.payload)
    if (!payload) return
    const attention = attentionKindForPayload(payload.kind)
    if (attention) {
      const requested = attention.endsWith('_requested')
      if (requested && address.agent === 'main') this.flushAssistant()
      this.sink.attention(attention, payload.value, address.agent)
      if (requested) this.sink.updateSession('waiting', attention === 'permission_requested'
        ? 'permission' : attention === 'question_requested' ? 'question' : 'plan')
      return
    }
    if (payload.kind === 'TurnDiffSummary' && isRecord(payload.value)) {
      const paths = Array.isArray(payload.value.modified_files)
        ? payload.value.modified_files.filter((path): path is string => typeof path === 'string')
        : []
      if (paths.length > 0) this.sink.artifacts?.(paths, address.agent)
      return
    }
    const notice = projectEventNotice(payload.kind, payload.value)
    if (notice) {
      const entry = {
        ...this.entry('system', notice.text, event.data.event_id),
        agentId: address.agent,
        eventNotice: notice.runtime ?? true,
      }
      if (address.agent === 'main') this.sink.append(entry)
      else this.sink.appendAgent?.(entry, address.agent)
    }
    if (payload.kind === 'Error') {
      if (address.agent === 'main') this.flushAssistant()
      const message = isRecord(payload.value) && typeof payload.value.message === 'string'
        ? payload.value.message
        : 'Loopal runtime failed'
      const entry = { ...this.entry('error', message, event.data.event_id), agentId: address.agent }
      if (address.agent === 'main') {
        this.sink.append(entry)
        this.failed = true
        this.sink.updateSession('failed', 'failure')
      } else this.sink.appendAgent?.(entry, address.agent)
      return
    }
    if (address.agent !== 'main') return
    if (payload.kind === 'Stream' && isRecord(payload.value)) {
      if (typeof payload.value.text === 'string') this.assistantBuffer += payload.value.text
      return
    }
    if (payload.kind === 'SessionHistoryLoaded') {
      this.sink.overflow?.()
      return
    }
    if (payload.kind === 'Running' || payload.kind === 'Started') {
      this.failed = false
      this.sink.updateSession('running')
      return
    }
    if (payload.kind === 'TurnCompleted') {
      this.flushAssistant()
      return
    }
    if (payload.kind === 'Interrupted' || payload.kind === 'TurnCancelled') {
      this.flushAssistant()
      if (!this.failed) this.sink.updateSession('waiting')
      return
    }
    if (payload.kind === 'AwaitingInput' || payload.kind === 'Finished') {
      this.flushAssistant()
      if (!this.failed) this.sink.updateSession('waiting', 'completed')
      return
    }
    if (payload.kind === 'ToolCall' && isRecord(payload.value)) {
      this.flushAssistant()
      const name = typeof payload.value.name === 'string' ? payload.value.name : 'tool'
      this.sink.append(this.entry('system', `Running ${name}`, event.data.event_id))
    }
  }

  private flushAssistant(): void {
    if (!this.assistantBuffer) return
    this.sink.append(this.entry('assistant', this.assistantBuffer))
    this.assistantBuffer = ''
  }

  private entry(
    role: ConversationEntry['role'],
    text: string,
    eventId?: number,
  ): ConversationEntry {
    return {
      id: eventId ? `event-${eventId}` : randomUUID(),
      role,
      text,
      createdAt: this.now().toISOString(),
    }
  }
}

function unpackPayload(value: unknown): { kind: string; value?: unknown } | undefined {
  if (typeof value === 'string') return { kind: value }
  if (!isRecord(value)) return undefined
  const entry = Object.entries(value)[0]
  return entry ? { kind: entry[0], value: entry[1] } : undefined
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}
