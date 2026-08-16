import { useEffect, useRef, useState } from 'react'
import {
  type LoopalDesktopAPI, type PermissionRequest, type QuestionRequest,
} from '../../../../shared/contracts'
import {
  attentionRequestId, permissionItem, questionIndex,
  questionItems, questionRequestId,
} from './attention-projector'
import {
  chooseAnswer, questionAnswers, questionDrafts, type QuestionAnswerDraft, writeOther,
} from './question-answer-draft'
import { errorMessage } from '../../../browser/runtime-controller-utils'
import {
  type SessionContextItem, type Stage2WorkbenchCallbacks,
  type Stage2WorkbenchModel,
} from '../../../browser/stage2-view-model'

interface AttentionBinding {
  readonly permissions: Stage2WorkbenchModel['permissions']
  readonly questions: Stage2WorkbenchModel['questions']
  readonly callbacks: Stage2WorkbenchCallbacks
}

export function useAttentionController(
  api: LoopalDesktopAPI,
  sessions: readonly SessionContextItem[],
  activeSessionId: string | undefined,
  enabled: boolean,
  reportError: (message: string | undefined) => void,
): AttentionBinding {
  const [permissions, setPermissions] = useState<readonly PermissionRequest[]>([])
  const [questions, setQuestions] = useState<readonly QuestionRequest[]>([])
  const answers = useRef(new Map<string, QuestionAnswerDraft[]>())
  const sessionsRef = useRef(sessions)
  sessionsRef.current = sessions
  const [, setAnswerVersion] = useState(0)
  const active = sessions.find((session) => session.id === activeSessionId)
  const runtimeKey = sessions.map((session) => (
    `${session.id}:${session.runtimeId ?? ''}:${session.runtimeGeneration ?? ''}`
  )).join('|')
  const isLive = (scope: RuntimeScope): boolean => scopeIsLive(sessions, scope)

  useEffect(() => {
    if (!enabled) return
    return api.onEvent((event) => {
      if (event.type === 'permission_requested'
        && scopeIsLive(sessionsRef.current, event.request)) {
        setPermissions((current) => upsert(current, event.request))
      } else if (event.type === 'question_requested'
        && scopeIsLive(sessionsRef.current, event.request)) {
        setQuestions((current) => upsert(current, event.request))
      } else if (event.type === 'permission_resolved') {
        setPermissions((current) => remove(current, event))
      } else if (event.type === 'question_resolved') {
        setQuestions((current) => remove(current, event))
        if (answers.current.delete(scopeKey(event))) setAnswerVersion((value) => value + 1)
      }
    })
  }, [api, enabled])

  useEffect(() => {
    setPermissions((current) => retainLive(current, isLive))
    setQuestions((current) => retainLive(current, isLive))
    const live = sessions.flatMap((session) => session.runtimeId
      && session.runtimeGeneration !== undefined
      ? [`${session.id}:${session.runtimeId}:${session.runtimeGeneration}:`]
      : [])
    let changed = false
    for (const key of answers.current.keys()) {
      if (!live.some((prefix) => key.startsWith(prefix))) {
        answers.current.delete(key)
        changed = true
      }
    }
    if (changed) setAnswerVersion((value) => value + 1)
  }, [runtimeKey])

  const run = (operation: () => Promise<void>): void => {
    reportError(undefined)
    void operation().catch((reason: unknown) => reportError(errorMessage(reason)))
  }
  const activeScope = (value: RuntimeScope): boolean => value.sessionId === activeSessionId
    && value.runtimeId === active?.runtimeId && value.generation === active?.runtimeGeneration

  return {
    permissions: permissions.filter(activeScope).map(permissionItem),
    questions: questions.filter(activeScope).flatMap((request) => (
      questionItems(request, answers.current.get(scopeKey(request)))
    )),
    callbacks: {
      onResolvePermission: (id, decision) => run(async () => {
        const request = permissions.find((item) => (
          attentionRequestId(item) === id && activeScope(item)
        ))
        if (!request) return
        const responseDecision = decision === 'allow' ? 'allow_once' : decision
        if (responseDecision !== 'deny' && !request.intentDigest) {
          throw new Error('Permission intent digest is required to allow a tool')
        }
        await api.respondPermission({
          ...requestScope(request), requestId: request.id,
          ...(request.intentDigest ? { intentDigest: request.intentDigest } : {}),
          decision: responseDecision,
        })
      }),
      onAnswerQuestion: (id, choiceId) => run(async () => {
        const request = requestForQuestion(questions, id, activeScope)
        if (!request) return
        const key = scopeKey(request)
        const current = questionDrafts(request, answers.current.get(key))
        const index = questionIndex(id)
        const selected = current[index]
        const question = request.questions[index]
        if (!selected || !question) return
        current[index] = chooseAnswer(question, selected, choiceId)
        answers.current.set(key, current)
        setAnswerVersion((value) => value + 1)
      }),
      onQuestionFreeTextChange: (id, value) => run(async () => {
        const request = requestForQuestion(questions, id, activeScope)
        if (!request) return
        const key = scopeKey(request)
        const current = questionDrafts(request, answers.current.get(key))
        const index = questionIndex(id)
        const draft = current[index]
        const question = request.questions[index]
        if (!draft || !question) return
        current[index] = writeOther(question, draft, value)
        answers.current.set(key, current)
        setAnswerVersion((version) => version + 1)
      }),
      onSubmitQuestionAnswers: (requestId) => run(async () => {
        const request = questions.find((item) => (
          attentionRequestId(item) === requestId && activeScope(item)
        ))
        if (!request) return
        const resolved = questionAnswers(request, answers.current.get(scopeKey(request)))
        if (resolved) await api.respondQuestion({
          ...requestScope(request), requestId: request.id, answers: resolved,
        })
      }),
      onCancelQuestion: (requestId) => run(async () => {
        const request = questions.find((item) => (
          attentionRequestId(item) === requestId && activeScope(item)
        ))
        if (request) await api.respondQuestion({
          ...requestScope(request), requestId: request.id, cancelled: true,
        })
      }),
    },
  }
}
interface RuntimeScope {
  readonly sessionId: string; readonly runtimeId: string; readonly generation: number
}
interface AttentionScope extends RuntimeScope { readonly agentId: string }
function requestScope(value: AttentionScope): AttentionScope {
  return {
    sessionId: value.sessionId, runtimeId: value.runtimeId, generation: value.generation,
    agentId: value.agentId,
  }
}
function scopeKey(value: AttentionScope & { readonly id?: string; readonly requestId?: string }): string {
  return [
    value.sessionId, value.runtimeId, value.generation,
    value.agentId, value.id ?? value.requestId,
  ].join(':')
}
function upsert<T extends AttentionScope & { readonly id: string }>(
  values: readonly T[], value: T,
): readonly T[] {
  const key = scopeKey(value)
  return values.some((item) => scopeKey(item) === key)
    ? values.map((item) => scopeKey(item) === key ? value : item)
    : [...values, value]
}
function remove<T extends AttentionScope & { readonly id: string }>(
  values: readonly T[], event: AttentionScope & { readonly requestId: string },
): readonly T[] {
  const key = scopeKey(event)
  return values.filter((item) => scopeKey(item) !== key)
}
function retainLive<T extends RuntimeScope>(
  values: readonly T[], live: (value: RuntimeScope) => boolean,
): readonly T[] {
  const next = values.filter(live)
  return next.length === values.length ? values : next
}
function requestForQuestion(
  requests: readonly QuestionRequest[], id: string,
  active: (value: RuntimeScope) => boolean,
): QuestionRequest | undefined {
  const requestId = questionRequestId(id)
  return requests.find((item) => attentionRequestId(item) === requestId && active(item))
}
function scopeIsLive(sessions: readonly SessionContextItem[], scope: RuntimeScope) {
  return sessions.some((session) => session.id === scope.sessionId
    && session.runtimeId === scope.runtimeId
    && session.runtimeGeneration === scope.generation)
}
