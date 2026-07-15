import { type AgentControlCommand, type SessionView } from '../../../../shared/contracts'
import { type MessageKey } from '../../../../shared/i18n'
import { useI18n } from '../../../browser/i18n-context'

interface TaskInspectorProps {
  readonly view: SessionView | undefined
  readonly canControl?: boolean
  readonly busy?: boolean
  readonly onControl?: (command: AgentControlCommand) => void
  readonly sections?: readonly TaskSection[]
  readonly testId?: string
  readonly showEmpty?: boolean
}

export type TaskSection = 'goal' | 'tasks' | 'background' | 'crons'

const goalStatusKeys: Record<NonNullable<SessionView['goal']>['status'], MessageKey> = {
  active: 'tasks.goalActive', paused: 'tasks.goalPaused',
  complete: 'tasks.goalComplete', infeasible: 'tasks.goalInfeasible',
}

export function TaskInspector(props: TaskInspectorProps): React.JSX.Element {
  const { locale, t } = useI18n()
  const { view } = props
  const disabled = !props.canControl || props.busy
  const run = (command: AgentControlCommand): void => props.onControl?.(command)
  const enabled = (section: TaskSection): boolean => props.sections?.includes(section) ?? true
  const hasWork = Boolean(
    (enabled('goal') && view?.goal)
    || (enabled('tasks') && view?.tasks.length)
    || (enabled('background') && view?.backgroundTasks.length)
    || (enabled('crons') && view?.crons.length),
  )
  const showEmpty = props.showEmpty ?? true
  return (
    <div className="inspector-content task-inspector" data-testid={props.testId ?? 'tasks-pane'}>
      {view?.goal && enabled('goal') && (
        <section className="inspector-section">
          <header className="inspector-section-heading">
            <h3>{t('tasks.goal')}</h3>
            <small>{t('tasks.observed')}</small>
          </header>
          <div className={`goal-card goal-${view.goal.status}`}>
            <span>{t(goalStatusKeys[view.goal.status])}</span>
            <p>{view.goal.objective}</p>
          </div>
        </section>
      )}
      {view && enabled('tasks') && view.tasks.length > 0 && (
        <section className="inspector-section">
          <h3>{t('tasks.planProgress', {
            completed: completed(view), total: view.tasks.length,
          })}</h3>
          <div className="task-list">
            {view.tasks.map((task) => (
              <details className={`task-row task-${task.status}`} key={task.id}>
                <summary>
                  <span className="task-state" />
                  <strong>{task.subject}</strong>
                  <small>#{task.id}</small>
                </summary>
                <div className="task-detail">
                  {task.activeForm && <p>{task.activeForm}</p>}
                  {task.description && <p>{task.description}</p>}
                  {task.blockedBy.length > 0 && <small>{t('tasks.blockedBy', { ids: task.blockedBy.join(', ') })}</small>}
                  {task.blocks.length > 0 && <small>{t('tasks.blocks', { ids: task.blocks.join(', ') })}</small>}
                </div>
              </details>
            ))}
          </div>
        </section>
      )}
      {view && enabled('background') && view.backgroundTasks.length > 0 && (
        <section className="inspector-section">
          <h3>{t('tasks.background')}</h3>
          {view.backgroundTasks.map((task) => (
            <article className={`background-task bg-${task.status}`} key={task.id}>
              <div className="resource-heading">
                <strong>{task.description}</strong><small>{task.status}</small>
                {task.status === 'running' && props.onControl && (
                  <button disabled={disabled} aria-label={t('tasks.killBackground', { name: task.description })} onClick={() => run({ type: 'background_task_kill', id: task.id })}>{t('tasks.kill')}</button>
                )}
              </div>
              {task.output && <pre>{task.output}</pre>}
              {task.exitCode !== null && <small>{t('tasks.exitCode', { code: task.exitCode })}</small>}
            </article>
          ))}
        </section>
      )}
      {view && enabled('crons') && view.crons.length > 0 && (
        <section className="inspector-section">
          <h3>{t('tasks.scheduled')}</h3>
          {view.crons.map((cron) => (
            <div className="cron-row" key={cron.id}>
              <div className="resource-heading">
                <strong>{cron.schedule || (cron.recurring ? t('tasks.recurring') : t('tasks.oneShot'))}</strong>
                {props.onControl && (
                  <button disabled={disabled} aria-label={t('tasks.deleteScheduled', { name: cron.prompt })} onClick={() => run({ type: 'cron_delete', id: cron.id })}>{t('tasks.delete')}</button>
                )}
              </div>
              <p>{cron.prompt}</p>
              <small>{cron.nextFireAt
                ? t('tasks.next', { time: new Date(cron.nextFireAt).toLocaleString(locale) })
                : t('tasks.exhausted')}</small>
            </div>
          ))}
        </section>
      )}
      {showEmpty && !hasWork && !props.onControl && (
        <div className="empty-inspector"><span>✓</span><p>{t('tasks.empty')}</p></div>
      )}
      {showEmpty && !hasWork && props.onControl && !view && (
        <div className="empty-inspector"><span>✓</span><p>{t('tasks.empty')}</p></div>
      )}
    </div>
  )
}

function completed(view: SessionView): number {
  return view.tasks.filter((task) => task.status === 'completed').length
}
