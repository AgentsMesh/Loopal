import { type PlanApprovalItem } from '../contrib/attention/browser/plan-approval-view-model'

export interface WorkspaceContextItem {
  readonly id: string
  readonly name: string
  readonly detail: string
}

export interface SessionContextItem {
  readonly id: string
  readonly workspaceId: string
  readonly title: string
  readonly state: string
  readonly runtimeId?: string
  readonly runtimeGeneration?: number
}

export interface PermissionItem {
  readonly id: string
  readonly agentId?: string
  readonly title: string
  readonly description: string
  readonly risk: 'low' | 'medium' | 'high'
  readonly command?: string
}

export interface QuestionChoice {
  readonly id: string
  readonly label: string
  readonly description?: string
}

export interface QuestionItem {
  readonly id: string
  readonly agentId?: string
  readonly prompt: string
  readonly allowMultiple: boolean
  readonly selectedChoiceIds: readonly string[]
  readonly otherText?: string
  readonly choices: readonly QuestionChoice[]
  readonly classifier?: {
    readonly kind: 'running' | 'failed' | 'completed'
    readonly label: string
  }
  readonly submit?: {
    readonly requestId: string
    readonly enabled: boolean
  }
}

export interface Stage2WorkbenchModel {
  readonly error?: string
  readonly context: {
    readonly workspaces: readonly WorkspaceContextItem[]
    readonly activeWorkspaceId?: string
    readonly sessions: readonly SessionContextItem[]
    readonly activeSessionId?: string
  }
  readonly permissions: readonly PermissionItem[]
  readonly questions: readonly QuestionItem[]
  readonly planApprovals: readonly PlanApprovalItem[]
}

export interface Stage2WorkbenchCallbacks {
  readonly onWorkspaceChange?: (id: string) => void
  readonly onSessionChange?: (id: string) => void
  readonly onResolvePermission?: (
    id: string, decision: 'allow' | 'allow_session' | 'deny'
  ) => void
  readonly onAnswerQuestion?: (id: string, choiceId: string) => void
  readonly onQuestionFreeTextChange?: (id: string, value: string) => void
  readonly onSubmitQuestionAnswers?: (requestId: string) => void
  readonly onCancelQuestion?: (requestId: string) => void
  readonly onPlanApprovalEdit?: (id: string, value: string) => void
  readonly onResolvePlanApproval?: (
    id: string, decision: 'approve' | 'reject' | 'approve_with_edits'
  ) => void
}

export interface Stage2WorkbenchBinding {
  readonly model: Stage2WorkbenchModel
  readonly callbacks: Stage2WorkbenchCallbacks
}

export const emptyStage2WorkbenchModel: Stage2WorkbenchModel = {
  context: { workspaces: [], sessions: [] },
  permissions: [],
  questions: [],
  planApprovals: [],
}
