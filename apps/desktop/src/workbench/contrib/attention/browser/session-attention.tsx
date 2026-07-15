import {
  type Stage2WorkbenchCallbacks,
  type Stage2WorkbenchModel,
} from '../../../browser/stage2-view-model'
import { AttentionPane } from './attention-pane'
import { PlanApprovalPane } from './plan-approval-pane'
import { useI18n } from '../../../browser/i18n-context'

export function SessionAttention(props: {
  readonly model: Stage2WorkbenchModel
  readonly callbacks: Stage2WorkbenchCallbacks
}): React.JSX.Element | null {
  const { t } = useI18n()
  const hasPermissions = props.model.permissions.length > 0
  const hasQuestions = props.model.questions.length > 0
  const hasPlans = props.model.planApprovals.length > 0
  if (!hasPermissions && !hasQuestions && !hasPlans) return null
  return (
    <section className="session-attention" aria-label={t('attention.requests')} data-testid="session-attention">
      {hasPermissions && (
        <AttentionPane
          kind="permissions" model={props.model} callbacks={props.callbacks} showEmpty={false}
        />
      )}
      {hasQuestions && (
        <AttentionPane
          kind="questions" model={props.model} callbacks={props.callbacks} showEmpty={false}
        />
      )}
      {hasPlans && <PlanApprovalPane model={props.model} callbacks={props.callbacks} />}
    </section>
  )
}
