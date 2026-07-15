import { useState } from 'react'
import { type LoopalDesktopAPI } from '../../shared/contracts'
import {
  type Stage2WorkbenchBinding, type Stage2WorkbenchModel,
} from './stage2-view-model'
import { useAttentionController } from '../contrib/attention/browser/use-attention-controller'
import { usePlanApprovalController } from '../contrib/attention/browser/use-plan-approval-controller'

export function useWorkbenchRuntimeController(
  api: LoopalDesktopAPI,
  context: Stage2WorkbenchModel['context'],
  enabled: boolean,
): Stage2WorkbenchBinding {
  const [error, setError] = useState<string>()
  const sessionId = context.activeSessionId
  const attention = useAttentionController(
    api, context.sessions, sessionId, enabled, setError,
  )
  const plans = usePlanApprovalController(
    api, context.sessions, sessionId, enabled, setError,
  )

  return {
    model: {
      context,
      ...(error !== undefined ? { error } : {}),
      permissions: attention.permissions,
      questions: attention.questions,
      planApprovals: plans.planApprovals,
    },
    callbacks: {
      ...attention.callbacks,
      ...plans.callbacks,
    },
  }
}
