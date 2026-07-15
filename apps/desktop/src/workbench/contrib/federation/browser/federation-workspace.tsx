import {
  type FederationConversationTarget, type FederationSnapshot,
} from './federation-model'
import { FederationTopology } from './federation-topology'
import { useI18n } from '../../../browser/i18n-context'

interface FederationWorkspaceProps {
  readonly snapshot: FederationSnapshot
  readonly busy?: string
  readonly error?: string
  readonly onStart: () => Promise<void>
  readonly onRefresh: () => Promise<void>
  readonly onOpenConversation: (target: FederationConversationTarget) => void
  readonly onManage: () => void
}

export function FederationWorkspace(
  props: FederationWorkspaceProps,
): React.JSX.Element {
  const { locale, t } = useI18n()
  const { local, network, connections } = props.snapshot
  const connectedSessions = connections.filter(({ state }) => state.state === 'connected').length
  if (network.state !== 'connected') {
    const running = local.state === 'running'
    return (
      <section className="federation-workspace federation-empty"
        data-testid="federation-workspace" data-workspace="federation">
        <div className="federation-empty-mark">⌁</div>
        <span className="eyebrow">{t('federation.title')}</span>
        <h2>{t(running ? 'federation.runningEmpty' : 'federation.notRunning')}</h2>
        <p>{t(running ? 'federation.runningEmptyHint' : 'federation.notRunningHint')}</p>
        <div className="federation-empty-actions">
          {!running && <button data-testid="federation-start"
            disabled={props.busy === 'start' || local.state === 'starting'}
            onClick={() => void props.onStart()}>{t(local.state === 'failed'
              ? 'federation.restart' : 'federation.start')}</button>}
          {running && <button onClick={() => void props.onRefresh()}>
            {t('federation.refresh')}
          </button>}
          <button className="secondary" onClick={props.onManage}>{t('federation.manage')}</button>
        </div>
        <small data-testid="federation-local-state">
          {t(`settings.metahub.status.${local.state}`)}
        </small>
        {(props.error || local.error || network.error) && <p className="diagnostic-error" role="alert">
          {props.error ?? local.error ?? network.error}
        </p>}
      </section>
    )
  }
  return (
    <section className="federation-workspace" data-testid="federation-workspace"
      data-workspace="federation">
      <header className="federation-overview">
        <div><span className="eyebrow">{t('federation.title')}</span>
          <h2>{t('federation.subtitle')}</h2></div>
        <div className="federation-overview-state" data-testid="federation-connection">
          <span className={`federation-state state-${network.state}`}>
            {t(`settings.metahub.status.${network.state}`)}
          </span>
          <strong>{t('federation.summary', {
            hubs: network.hubs.length, agents: network.topology.length,
          })} · {t('federation.sessions', { count: connectedSessions })}</strong>
          <small>{network.address ?? network.hubName ?? '—'} · {t('federation.updated', {
            value: new Intl.DateTimeFormat(locale, {
              hour: '2-digit', minute: '2-digit', second: '2-digit',
            }).format(new Date(network.refreshedAt)),
          })}</small>
          <div className="federation-header-actions">
            <button onClick={() => void props.onRefresh()}>{t('federation.refresh')}</button>
            <button onClick={props.onManage}>{t('federation.manage')}</button>
          </div>
        </div>
      </header>
      {(props.error || network.error) && <div className="federation-error" role="alert">
        {props.error ?? t('federation.error', { error: network.error ?? '' })}
      </div>}
      <FederationTopology state={network} topology={props.snapshot.topology}
        onOpenConversation={props.onOpenConversation} />
    </section>
  )
}
