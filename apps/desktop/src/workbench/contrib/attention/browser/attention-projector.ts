import {
  type PermissionRequest,
  type QuestionRequest,
} from '../../../../shared/contracts'
import {
  type PermissionItem,
  type QuestionItem,
} from '../../../browser/stage2-view-model'
import {
  choiceAnswer, questionAnswers, type QuestionAnswerDraft,
} from './question-answer-draft'

export function permissionItem(request: PermissionRequest): PermissionItem {
  return {
    id: attentionRequestId(request),
    agentId: request.agentId,
    title: request.title,
    description: request.detail,
    risk: request.risk,
    canAllow: request.intentDigest !== undefined,
    command: request.tool,
  }
}

export function questionItems(
  request: QuestionRequest,
  selected: readonly QuestionAnswerDraft[] = [],
): readonly QuestionItem[] {
  const ready = questionAnswers(request, selected) !== undefined
  const requestId = attentionRequestId(request)
  return request.questions.map((question, index) => ({
    id: `${requestId}:${index}`,
    agentId: request.agentId,
    prompt: question.header
      ? `${question.header}: ${question.question}`
      : question.question,
    allowMultiple: question.allowMultiple,
    selectedChoiceIds: selected[index]?.selected ?? [],
    otherText: selected[index]?.other ?? '',
    choices: question.options.map((option, optionIndex) => ({
      id: `${optionIndex}:${option.label}`,
      label: option.label,
      ...(option.description ? { description: option.description } : {}),
    })),
    ...(index === 0 && classifierItem(request) ? { classifier: classifierItem(request)! } : {}),
    ...(index === request.questions.length - 1
      ? { submit: { requestId, enabled: ready } }
      : {}),
  }))
}

function classifierItem(request: QuestionRequest): QuestionItem['classifier'] {
  const status = request.classifierStatus
  if ((!status || status.kind === 'none') && !request.classifierRunning) return undefined
  if (!status || status.kind === 'running') {
    const seconds = ((status?.elapsedMs ?? 0) / 1_000).toFixed(1)
    return { kind: 'running', label: `Auto-answering · ${seconds}s` }
  }
  if (status.kind === 'failed') {
    return { kind: 'failed', label: `Auto-answer unavailable · ${status.reason || 'unknown error'}` }
  }
  return { kind: 'completed', label: 'Auto-answer ready' }
}

export function questionRequestId(id: string): string {
  return id.slice(0, id.lastIndexOf(':'))
}

export function questionIndex(id: string): number {
  return Number(id.slice(id.lastIndexOf(':') + 1))
}

export function questionAnswer(choiceId: string): string {
  return choiceAnswer(choiceId)
}

export function attentionRequestId(request: {
  readonly agentId: string
  readonly id: string
}): string {
  return `${request.agentId}:${request.id}`
}
