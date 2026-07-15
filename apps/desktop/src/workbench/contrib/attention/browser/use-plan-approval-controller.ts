import { useEffect, useRef, useState } from 'react'
import {
  type LoopalDesktopAPI, type PlanApprovalRequest,
} from '../../../../shared/contracts'
import { attentionRequestId } from './attention-projector'
import { errorMessage } from '../../../browser/runtime-controller-utils'
import {
  type SessionContextItem, type Stage2WorkbenchCallbacks, type Stage2WorkbenchModel,
} from '../../../browser/stage2-view-model'

interface RuntimeScope {
  readonly sessionId: string
  readonly runtimeId: string
  readonly generation: number
}

export function usePlanApprovalController(
  api: LoopalDesktopAPI,
  sessions: readonly SessionContextItem[],
  activeSessionId: string | undefined,
  enabled: boolean,
  reportError: (message: string | undefined) => void,
): {
  readonly planApprovals: Stage2WorkbenchModel['planApprovals']
  readonly callbacks: Stage2WorkbenchCallbacks
} {
  const [requests, setRequests] = useState<readonly PlanApprovalRequest[]>([])
  const [drafts, setDrafts] = useState<Readonly<Record<string, string>>>({})
  const sessionsRef = useRef(sessions)
  sessionsRef.current = sessions
  const active = sessions.find((session) => session.id === activeSessionId)
  const runtimeKey = sessions.map((session) => (
    `${session.id}:${session.runtimeId ?? ''}:${session.runtimeGeneration ?? ''}`
  )).join('|')
  const isLive = (scope: RuntimeScope): boolean => sessions.some((session) => (
    session.id === scope.sessionId && session.runtimeId === scope.runtimeId
      && session.runtimeGeneration === scope.generation
  ))
  const activeScope = (scope: RuntimeScope): boolean => scope.sessionId === activeSessionId
    && scope.runtimeId === active?.runtimeId && scope.generation === active?.runtimeGeneration

  useEffect(() => {
    if (!enabled) return
    return api.onEvent((event) => {
      if (event.type === 'plan_approval_requested'
        && scopeIsLive(sessionsRef.current, event.request)) {
        setRequests((current) => upsert(current, event.request))
        setDrafts((current) => current[key(event.request)] === undefined
          ? { ...current, [key(event.request)]: event.request.planContent } : current)
      } else if (event.type === 'plan_approval_resolved') {
        setRequests((current) => current.filter((item) => key(item) !== key(event)))
        setDrafts((current) => omit(current, key(event)))
      }
    })
  }, [api, enabled])

  useEffect(() => {
    setRequests((current) => current.filter(isLive))
    setDrafts((current) => Object.fromEntries(Object.entries(current).filter(([draftKey]) => (
      sessions.some((session) => draftKey.startsWith(
        `${session.id}:${session.runtimeId}:${session.runtimeGeneration}:`,
      ))
    ))))
  }, [runtimeKey])

  const resolve = (id: string, decision: 'approve' | 'reject' | 'approve_with_edits'): void => {
    const request = requests.find((item) => attentionRequestId(item) === id && activeScope(item))
    if (!request) return
    reportError(undefined)
    void api.respondPlanApproval({
      ...scope(request), requestId: request.id, decision,
      ...(decision === 'approve_with_edits' ? { editedPlan: drafts[key(request)] ?? '' } : {}),
    }).catch((reason: unknown) => reportError(errorMessage(reason)))
  }
  return {
    planApprovals: requests.filter(activeScope).map((request) => ({
      id: attentionRequestId(request), agentId: request.agentId,
      path: request.planPath, content: request.planContent,
      editedContent: drafts[key(request)] ?? request.planContent,
    })),
    callbacks: {
      onPlanApprovalEdit: (id, value) => {
        const request = requests.find((item) => attentionRequestId(item) === id && activeScope(item))
        if (request) setDrafts((current) => ({ ...current, [key(request)]: value }))
      },
      onResolvePlanApproval: resolve,
    },
  }
}

function key(value: RuntimeScope & { readonly agentId: string; readonly id?: string;
  readonly requestId?: string }): string {
  return [value.sessionId, value.runtimeId, value.generation,
    value.agentId, value.id ?? value.requestId].join(':')
}
function scope(value: PlanApprovalRequest) {
  return { sessionId: value.sessionId, runtimeId: value.runtimeId,
    generation: value.generation, agentId: value.agentId }
}
function upsert(values: readonly PlanApprovalRequest[], value: PlanApprovalRequest) {
  return values.some((item) => key(item) === key(value))
    ? values.map((item) => key(item) === key(value) ? value : item) : [...values, value]
}
function omit(values: Readonly<Record<string, string>>, name: string) {
  const { [name]: _removed, ...rest } = values
  return rest
}
function scopeIsLive(sessions: readonly SessionContextItem[], scope: RuntimeScope) {
  return sessions.some((session) => session.id === scope.sessionId
    && session.runtimeId === scope.runtimeId
    && session.runtimeGeneration === scope.generation)
}
