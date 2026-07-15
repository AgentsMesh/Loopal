import { type DesktopEvent } from '../../../../shared/contracts'
import {
  projectAttentionEvent,
  type AttentionEventKind,
  type ProjectedAttentionEvent,
} from './loopal-attention'
import { type SnapshotAttention } from '../sessions/loopal-session-snapshot'
import { type SessionRuntimeScope } from '../runtime/session-runtime-registry'

interface PendingRequest {
  readonly requestId: string
  readonly agentId: string
}

export class LoopalLiveAttention {
  private readonly permissions = new Map<string, PendingRequest>()
  private readonly questions = new Map<string, PendingRequest>()
  private readonly plans = new Map<string, PendingRequest>()

  constructor(
    private readonly scope: SessionRuntimeScope,
    private readonly now: () => Date,
    private readonly fire: (event: DesktopEvent) => void,
  ) {}

  accept(kind: AttentionEventKind, value: unknown, agentId: string): void {
    const event = projectAttentionEvent(kind, value, this.scope, agentId, this.now)
    if (!event) return
    this.track(event)
    this.fire(event)
  }

  reconcile(pending: readonly SnapshotAttention[]): void {
    const events = pending.flatMap((item) => {
      const event = projectAttentionEvent(
        item.kind, item.value, this.scope, item.agentId, this.now,
      )
      return event ? [event] : []
    })
    const permissionKeys = new Set(events.flatMap((event) => (
      event.type === 'permission_requested'
        ? [key(event.request.agentId, event.request.id)]
        : []
    )))
    const questionKeys = new Set(events.flatMap((event) => (
      event.type === 'question_requested'
        ? [key(event.request.agentId, event.request.id)]
        : []
    )))
    const planKeys = new Set(events.flatMap((event) => (
      event.type === 'plan_approval_requested'
        ? [key(event.request.agentId, event.request.id)]
        : []
    )))
    this.resolveMissing(this.permissions, permissionKeys, 'permission_resolved')
    this.resolveMissing(this.questions, questionKeys, 'question_resolved')
    this.resolveMissing(this.plans, planKeys, 'plan_approval_resolved')
    for (const event of events) {
      this.track(event)
      this.fire(event)
    }
  }

  retire(): readonly DesktopEvent[] {
    const events = [
      ...this.resolvedEvents(this.permissions, 'permission_resolved'),
      ...this.resolvedEvents(this.questions, 'question_resolved'),
      ...this.resolvedEvents(this.plans, 'plan_approval_resolved'),
    ]
    this.permissions.clear()
    this.questions.clear()
    this.plans.clear()
    return events
  }

  private track(event: ProjectedAttentionEvent): void {
    if (event.type === 'permission_requested') {
      this.permissions.set(key(event.request.agentId, event.request.id), {
        requestId: event.request.id, agentId: event.request.agentId,
      })
    } else if (event.type === 'question_requested') {
      this.questions.set(key(event.request.agentId, event.request.id), {
        requestId: event.request.id, agentId: event.request.agentId,
      })
    } else if (event.type === 'plan_approval_requested') {
      this.plans.set(key(event.request.agentId, event.request.id), {
        requestId: event.request.id, agentId: event.request.agentId,
      })
    } else if (event.type === 'permission_resolved') {
      this.permissions.delete(key(event.agentId, event.requestId))
    } else if (event.type === 'question_resolved') {
      this.questions.delete(key(event.agentId, event.requestId))
    } else this.plans.delete(key(event.agentId, event.requestId))
  }

  private resolveMissing(
    requests: Map<string, PendingRequest>,
    desired: ReadonlySet<string>,
    type: 'permission_resolved' | 'question_resolved' | 'plan_approval_resolved',
  ): void {
    for (const [requestKey, request] of requests) {
      if (desired.has(requestKey) || request.agentId.includes('/')) continue
      this.fire({ type, ...this.scopeFields(), ...request })
      requests.delete(requestKey)
    }
  }

  private resolvedEvents(
    requests: ReadonlyMap<string, PendingRequest>,
    type: 'permission_resolved' | 'question_resolved' | 'plan_approval_resolved',
  ): DesktopEvent[] {
    return [...requests.values()].map((request) => ({
      type, ...this.scopeFields(), ...request,
    }))
  }

  private scopeFields() {
    return {
      sessionId: this.scope.sessionId,
      runtimeId: this.scope.runtimeId,
      generation: this.scope.generation,
    }
  }
}

function key(agentId: string, requestId: string): string {
  return `${agentId}\0${requestId}`
}
