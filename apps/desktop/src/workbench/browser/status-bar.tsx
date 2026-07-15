import { type SessionSummary } from '../../shared/contracts'
import { isSessionLive } from '../../shared/contracts/session-lifecycle'
import { type FederationSnapshot } from '../contrib/federation/browser/federation-model'
import { useI18n } from './i18n-context'

export function StatusBar({ sessions, federation, onOpenFederation }: {
  readonly sessions: readonly SessionSummary[]
  readonly federation: FederationSnapshot
  readonly onOpenFederation: () => void
}): React.JSX.Element {
  const { t } = useI18n()
  const running = sessions.filter(isSessionLive).length
  const attention = sessions.filter((session) => (
    session.attention && session.attention !== 'completed'
  )).length
  const federationState = federation.network.state === 'connected'
    || federation.network.state === 'error'
    ? federation.network.state
    : federation.local.state === 'running' ? 'running' : 'disconnected'
  return (
    <footer className="status-bar">
      <span>Loopal Desktop</span>
      <span>{t('status.running', { count: running })}</span>
      <span>{t('status.attention', { count: attention })}</span>
      <button className={`status-federation state-${federationState}`}
        aria-label={t('status.openFederation')} onClick={onOpenFederation}>
        <span />{t(`status.federation.${federationState}`)}
        {federation.network.state === 'connected' && <small>
          {t('federation.summary', {
            hubs: federation.network.hubs.length, agents: federation.network.topology.length,
          })}
        </small>}
      </button>
    </footer>
  )
}
