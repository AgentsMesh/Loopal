import {
  type Stage2WorkbenchCallbacks,
  type Stage2WorkbenchModel,
} from '../../../browser/stage2-view-model'
import { useI18n } from '../../../browser/i18n-context'

interface AttentionPaneProps {
  readonly kind: 'permissions' | 'questions'
  readonly model: Stage2WorkbenchModel
  readonly callbacks: Stage2WorkbenchCallbacks
  readonly showEmpty?: boolean
}

export function AttentionPane(props: AttentionPaneProps): React.JSX.Element {
  const { t } = useI18n()
  if (props.kind === 'permissions') {
    return (
      <div className="inspector-content attention-pane" data-testid="permissions-pane">
        {props.model.permissions.map((request) => (
          <article className={`attention-card risk-${request.risk}`} key={request.id}>
            <span className="attention-kind">
              {request.agentId
                ? t('attention.riskAgent', { risk: request.risk, agent: request.agentId })
                : t('attention.risk', { risk: request.risk })}
            </span>
            <h3>{request.title}</h3>
            <p>{request.description}</p>
            {request.command && <code>{request.command}</code>}
            <div className="attention-actions">
              <button onClick={() => props.callbacks.onResolvePermission?.(request.id, 'deny')}>
                {t('attention.deny')}
              </button>
              <button
                className="primary"
                disabled={!request.canAllow}
                onClick={() => props.callbacks.onResolvePermission?.(request.id, 'allow')}
              >
                {t('attention.allow')}
              </button>
              <button
                disabled={!request.canAllow}
                onClick={() => props.callbacks.onResolvePermission?.(request.id, 'allow_session')}
              >
                {t('attention.allowSession')}
              </button>
            </div>
          </article>
        ))}
        {(props.showEmpty ?? true) && props.model.permissions.length === 0 && (
          <AttentionEmpty label={t('attention.noApprovals')} />
        )}
      </div>
    )
  }

  return (
    <div className="inspector-content attention-pane" data-testid="questions-pane">
      {props.model.questions.map((question) => (
        <article className="attention-card" key={question.id}>
          <span className="attention-kind">
            {question.agentId
              ? t('attention.questionAgent', { agent: question.agentId })
              : t('attention.question')}
          </span>
          {question.classifier && (
            <span className={`classifier-status classifier-${question.classifier.kind}`}>
              {question.classifier.label}
            </span>
          )}
          <h3>{question.prompt}</h3>
          <div className="question-choices">
            {question.choices.map((choice) => (
              <button
                key={choice.id}
                className={question.selectedChoiceIds.includes(choice.id) ? 'selected' : ''}
                aria-pressed={question.selectedChoiceIds.includes(choice.id)}
                onClick={() => props.callbacks.onAnswerQuestion?.(question.id, choice.id)}
              >
                <strong>{choice.label}</strong>
                {choice.description && <small>{choice.description}</small>}
              </button>
            ))}
          </div>
          <label className="question-other">
            <span>{t('attention.other')}</span>
            <input
              aria-label={t('attention.otherAnswer', { prompt: question.prompt })}
              maxLength={100_000}
              placeholder={t('attention.customAnswer')}
              value={question.otherText ?? ''}
              onChange={(event) => props.callbacks.onQuestionFreeTextChange?.(
                question.id, event.target.value,
              )}
            />
          </label>
          {question.submit && (
            <div className="attention-actions">
              <button onClick={() => props.callbacks.onCancelQuestion?.(
                question.submit!.requestId,
              )}>
                {t('common.cancel')}
              </button>
              <button
                className="primary"
                disabled={!question.submit.enabled}
                onClick={() => props.callbacks.onSubmitQuestionAnswers?.(
                  question.submit!.requestId,
                )}
              >
                {t('attention.submit')}
              </button>
            </div>
          )}
        </article>
      ))}
      {(props.showEmpty ?? true) && props.model.questions.length === 0 && (
        <AttentionEmpty label={t('attention.noQuestions')} />
      )}
    </div>
  )
}

function AttentionEmpty({ label }: { readonly label: string }): React.JSX.Element {
  return <div className="empty-inspector"><span>✓</span><p>{label}</p></div>
}
