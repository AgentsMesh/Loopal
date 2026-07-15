import { useState } from 'react'
import {
  type AgentSummary, type SessionDetail,
} from '../../../../shared/contracts'
import { useI18n } from '../../../browser/i18n-context'

interface SessionToolbarProps {
  readonly detail?: SessionDetail
  readonly selectedAgent?: AgentSummary
}

export function SessionToolbar(props: SessionToolbarProps): React.JSX.Element {
  const { t } = useI18n()
  const [detailsOpen, setDetailsOpen] = useState(false)
  return <>
    <header className="session-toolbar">
      <div className="session-identity">
        <div className="session-title-row">
          <h2 data-testid="active-session-title">
            {props.detail?.session.title ?? t('workspace.selectSession')}
          </h2>
          {props.selectedAgent && <span className="session-agent-route">
            <span>/</span>{props.selectedAgent.name}
            {props.selectedAgent.qualifiedName && <small>
              @{props.selectedAgent.hubPath?.join('/') ?? props.selectedAgent.qualifiedName}
            </small>}
          </span>}
        </div>
      </div>
      <div className="toolbar-actions">
        <button aria-label={t('workspace.details')} aria-expanded={detailsOpen}
          aria-controls="session-metadata" disabled={!props.detail}
          onClick={() => setDetailsOpen((open) => !open)}>•••</button>
      </div>
    </header>
    {detailsOpen && props.detail && (
      <div className="session-details" id="session-metadata" data-testid="session-metadata">
        <dl className="session-metadata">
          <div><dt>{t('workspace.session')}</dt><dd>{props.detail.session.id}</dd></div>
          <div><dt>{t('workspace.model')}</dt><dd>
            {props.selectedAgent?.model ?? props.detail.session.model}
          </dd></div>
          <div><dt>{t('workspace.mode')}</dt><dd>
            {props.selectedAgent?.mode ?? props.detail.session.mode}
          </dd></div>
          <div><dt>{t('workspace.status')}</dt><dd>{props.detail.session.status}</dd></div>
        </dl>
      </div>
    )}
  </>
}
