import { type SessionView, type WorkflowRunSummary } from '../../../../shared/contracts'
import { useI18n } from '../../../browser/i18n-context'

export function WorkflowInspector(props: {
  readonly view: SessionView | undefined
}): React.JSX.Element {
  const { t } = useI18n()
  const active = props.view?.workflows.active ?? []
  const recent = props.view?.workflows.recent ?? []
  return (
    <div className="inspector-content task-inspector" data-testid="workflows-pane">
      {active.length > 0 && (
        <section className="inspector-section">
          <h3>{t('workflows.active')}</h3>
          <div className="task-list">{active.map((run) => (
            <WorkflowRow key={run.id} run={run} />
          ))}</div>
        </section>
      )}
      {recent.length > 0 && (
        <section className="inspector-section">
          <h3>{t('workflows.recent')}</h3>
          <div className="task-list">{recent.map((run) => (
            <WorkflowRow key={run.id} run={run} />
          ))}</div>
        </section>
      )}
    </div>
  )
}

function WorkflowRow({ run }: { readonly run: WorkflowRunSummary }): React.JSX.Element {
  const { t } = useI18n()
  const completed = run.counts.succeeded + run.counts.failed
    + run.counts.cancelled + run.counts.skipped
  const total = completed + run.counts.pending + run.counts.ready + run.counts.active
  return (
    <details className={`task-row workflow-${run.state}`}>
      <summary>
        <span className="task-state" />
        <strong>{run.runGoal}</strong>
        <small>{t('workflows.state', { state: run.state })}</small>
      </summary>
      <div className="task-detail">
        <p>{t('workflows.progress', { completed, total })}</p>
        <small>{t('workflows.revision', { revision: run.revision, id: run.id })}</small>
      </div>
    </details>
  )
}
