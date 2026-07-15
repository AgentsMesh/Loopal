import { useState } from 'react'
import { type MetaHubRuntimeState } from '../../../../shared/contracts'
import { useI18n } from '../../../browser/i18n-context'

export function MetaHubTopologyPanel(props: {
  readonly state?: MetaHubRuntimeState | undefined
  readonly selectedAgentId?: string | undefined
  readonly onSelectAgent?: ((agentId: string) => void) | undefined
  readonly onManage?: (() => void) | undefined
}): React.JSX.Element {
  const { t } = useI18n()
  const [requestedHub, setRequestedHub] = useState<string>()
  const state = props.state
  if (!state || state.state === 'disconnected') {
    return <p className="muted">{t('settings.metahub.topology.empty')}</p>
  }
  const hub = state.hubs.some((item) => item.name === requestedHub) ? requestedHub : undefined
  const topology = hub ? state.topology.filter((agent) => agent.hub === hub) : state.topology
  return (
    <div className="metahub-topology" data-testid="metahub-topology">
      <div className="metahub-panel-toolbar">
        <span>{t('settings.metahub.topology.summary', {
          hubs: state.hubs.length, agents: state.topology.length,
        })}</span>
        {props.onManage && <button onClick={props.onManage}>
          {t('settings.metahub.topology.manage')}
        </button>}
      </div>
      <div className="metahub-hubs" role="group"
        aria-label={t('settings.metahub.topology.filter')}>
        {state.hubs.map((item) => (
          <button
            className="metahub-hub" key={item.name} aria-pressed={hub === item.name}
            onClick={() => setRequestedHub((current) => current === item.name ? undefined : item.name)}
          >
            <span className={`agent-state agent-${item.status === 'connected' ? 'running' : 'waiting'}`} />
            <strong>{item.name}</strong>
            <small>{t(`settings.metahub.status.${item.status}`)} ·
              {' '}{t('settings.metahub.topology.agents', { count: item.agentCount })}</small>
          </button>
        ))}
      </div>
      {topology.length > 0 && (
        <div className="metahub-agent-paths">
          {topology.map((agent) => (
            <button
              className="metahub-agent" key={agent.id}
              data-agent-id={agent.id}
              aria-pressed={props.selectedAgentId === agent.id}
              onClick={() => props.onSelectAgent?.(agent.id)}
            >
              <code>{agent.id}</code>
              <small>{t(`settings.metahub.topology.lifecycle.${agent.lifecycle}`)}
                {agent.parentId ? ` · ← ${agent.parentId}` : ''}</small>
            </button>
          ))}
        </div>
      )}
      {state.error && <p className="diagnostic-error">{state.error}</p>}
    </div>
  )
}
