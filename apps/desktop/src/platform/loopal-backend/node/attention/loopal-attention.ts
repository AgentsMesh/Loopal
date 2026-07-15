import { AttentionDesktopEventSchema, type DesktopEvent } from '../../../../shared/contracts'
import { type SessionRuntimeScope } from '../runtime/session-runtime-registry'

export type AttentionEventKind =
  | 'permission_requested'
  | 'permission_resolved'
  | 'question_requested'
  | 'question_resolved'
  | 'plan_approval_requested'
  | 'plan_approval_resolved'
export type ProjectedAttentionEvent = Extract<DesktopEvent, { type: AttentionEventKind }>

export function projectAttentionEvent(
  kind: AttentionEventKind,
  value: unknown,
  scope: SessionRuntimeScope,
  agentId: string,
  now: () => Date,
): ProjectedAttentionEvent | undefined {
  if (!isRecord(value) || typeof value.id !== 'string') return undefined
  const event = kind === 'permission_requested'
    ? permissionEvent(value, scope, agentId, now)
    : kind === 'question_requested'
      ? questionEvent(value, scope, agentId, now)
      : kind === 'plan_approval_requested'
        ? planApprovalEvent(value, scope, agentId, now)
        : { type: kind, ...scopeFields(scope), agentId, requestId: value.id }
  const parsed = AttentionDesktopEventSchema.safeParse(event)
  return parsed.success ? parsed.data as ProjectedAttentionEvent : undefined
}

function planApprovalEvent(
  value: Record<string, unknown>,
  scope: SessionRuntimeScope,
  agentId: string,
  now: () => Date,
) {
  return {
    type: 'plan_approval_requested' as const,
    request: {
      id: value.id,
      ...scopeFields(scope),
      agentId,
      planContent: String(value.plan_content ?? ''),
      planPath: String(value.plan_path ?? ''),
      createdAt: now().toISOString(),
    },
  }
}

export function attentionKindForPayload(kind: string): AttentionEventKind | undefined {
  const kinds: Record<string, AttentionEventKind> = {
    ToolPermissionRequest: 'permission_requested',
    ToolPermissionResolved: 'permission_resolved',
    UserQuestionRequest: 'question_requested',
    UserQuestionResolved: 'question_resolved',
    PlanApprovalRequest: 'plan_approval_requested',
    PlanApprovalResolved: 'plan_approval_resolved',
  }
  return kinds[kind]
}

function permissionEvent(
  value: Record<string, unknown>,
  scope: SessionRuntimeScope,
  agentId: string,
  now: () => Date,
) {
  const tool = typeof value.name === 'string' ? value.name : 'tool'
  return {
    type: 'permission_requested' as const,
    request: {
      id: value.id,
      ...scopeFields(scope),
      agentId,
      tool,
      title: `Allow ${tool}`,
      detail: stringify(value.input),
      risk: permissionRisk(tool),
      createdAt: now().toISOString(),
    },
  }
}

function questionEvent(
  value: Record<string, unknown>,
  scope: SessionRuntimeScope,
  agentId: string,
  now: () => Date,
) {
  const classifier = classifierStatus(value)
  const questions = Array.isArray(value.questions)
    ? value.questions.filter(isRecord).map((question) => ({
      question: typeof question.question === 'string' ? question.question : 'Question',
      header: typeof question.header === 'string' ? question.header : undefined,
      options: Array.isArray(question.options)
        ? question.options.filter(isRecord).map((option) => ({
          label: String(option.label ?? ''),
          description: String(option.description ?? ''),
        })).filter((option) => option.label.length > 0)
        : [],
      allowMultiple: question.allow_multiple === true,
    }))
    : []
  return {
    type: 'question_requested' as const,
    request: {
      id: value.id,
      ...scopeFields(scope),
      agentId,
      questions,
      classifierRunning: value.classifier_running === true,
      ...(classifier ? { classifierStatus: classifier } : {}),
      createdAt: now().toISOString(),
    },
  }
}

function classifierStatus(value: Record<string, unknown>) {
  const status = isRecord(value.classifier_status) ? value.classifier_status : undefined
  const kind = typeof status?.kind === 'string' ? status.kind : undefined
  if (kind === 'running') {
    return { kind, elapsedMs: number(status?.elapsed_ms) }
  }
  if (kind === 'failed') return { kind, reason: String(status?.reason ?? '') }
  if (kind === 'completed') {
    const answers = Array.isArray(status?.answers) ? status.answers.map(String) : []
    return { kind, answers }
  }
  if (kind === 'none') return { kind }
  return value.classifier_running === true ? { kind: 'running' as const, elapsedMs: 0 } : undefined
}

function number(value: unknown): number {
  return typeof value === 'number' && Number.isFinite(value) ? Math.max(0, value) : 0
}

function scopeFields(scope: SessionRuntimeScope) {
  return {
    sessionId: scope.sessionId,
    runtimeId: scope.runtimeId,
    generation: scope.generation,
  }
}

function permissionRisk(tool: string): 'low' | 'medium' | 'high' {
  if (/delete|write|edit|bash|process/i.test(tool)) return 'high'
  if (/read|glob|grep|search/i.test(tool)) return 'low'
  return 'medium'
}

function stringify(value: unknown): string {
  if (typeof value === 'string') return value
  try {
    return JSON.stringify(value, null, 2)
  } catch {
    return String(value)
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}
