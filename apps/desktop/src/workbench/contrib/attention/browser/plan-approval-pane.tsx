import {
  type Stage2WorkbenchCallbacks, type Stage2WorkbenchModel,
} from '../../../browser/stage2-view-model'
import { useI18n } from '../../../browser/i18n-context'

export function PlanApprovalPane(props: {
  readonly model: Stage2WorkbenchModel
  readonly callbacks: Stage2WorkbenchCallbacks
}): React.JSX.Element {
  const { t } = useI18n()
  return (
    <div className="inspector-content attention-pane" data-testid="plan-approvals-pane">
      {props.model.planApprovals.map((request) => (
        <article
          className="attention-card plan-approval-card"
          data-testid="plan-approval-card"
          data-request-id={request.id}
          key={request.id}
        >
          <span className="attention-kind">{t('plan.kind', { agent: request.agentId })}</span>
          <h3>{t('plan.review')}</h3>
          <code data-testid="plan-approval-path">{request.path}</code>
          <pre data-testid="plan-approval-content">{request.content}</pre>
          <label className="plan-approval-edit">
            <span>{t('plan.edit')}</span>
            <textarea
              data-testid="plan-approval-editor"
              aria-label={t('plan.edited')}
              maxLength={1_000_000}
              value={request.editedContent}
              onChange={(event) => props.callbacks.onPlanApprovalEdit?.(
                request.id, event.target.value,
              )}
            />
          </label>
          <div className="attention-actions">
            <button
              data-testid="plan-approval-reject"
              onClick={() => props.callbacks.onResolvePlanApproval?.(request.id, 'reject')}
            >{t('plan.reject')}</button>
            <button
              className="primary"
              data-testid="plan-approval-approve"
              onClick={() => props.callbacks.onResolvePlanApproval?.(request.id, 'approve')}
            >{t('plan.approve')}</button>
            <button
              data-testid="plan-approval-approve-edits"
              onClick={() => props.callbacks.onResolvePlanApproval?.(
                request.id, 'approve_with_edits',
              )}
            >{t('plan.approveEdits')}</button>
          </div>
        </article>
      ))}
    </div>
  )
}
