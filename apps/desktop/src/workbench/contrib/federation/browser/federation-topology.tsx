import { useState } from 'react'
import {
  type MetaHubInfo, type MetaHubRuntimeState, type MetaHubTopologyAgent,
} from '../../../../shared/contracts'
import { type MessageKey } from '../../../../shared/i18n'
import {
  type FederationTopologyNode, type FederationConversationTarget,
} from './federation-model'
import { useI18n } from '../../../browser/i18n-context'

export function FederationTopology(props: {
  readonly state: MetaHubRuntimeState
  readonly topology: readonly FederationTopologyNode[]
  readonly onOpenConversation: (target: FederationConversationTarget) => void
}): React.JSX.Element {
  const { t } = useI18n()
  const [hubRequest, setHubRequest] = useState<string>()
  const [agentRequest, setAgentRequest] = useState<string>()
  const hub = props.state.hubs.some((item) => item.name === hubRequest)
    ? hubRequest : undefined
  const agents = hub
    ? props.topology.filter(({ agent }) => agent.hub === hub)
    : props.topology
  const selected = agents.find((node) => nodeKey(node) === agentRequest) ?? agents[0]
  return <div className="federation-layout">
    <aside className="federation-hub-list" data-testid="federation-hub-list"
      aria-label={t('federation.hubList')}>
      <div className="federation-section-label"><span>{t('federation.hubs')}</span></div>
      <button className="federation-hub" aria-pressed={!hub}
        onClick={() => setHubRequest(undefined)}>
        <strong>{t('federation.allHubs')}</strong>
        <small>{t('federation.agents', { count: props.state.topology.length })}</small>
      </button>
      {props.state.hubs.map((item) => <HubCard key={item.name} hub={item}
        selected={hub === item.name} onSelect={() => setHubRequest(item.name)} />)}
    </aside>
    <main className="federation-map" data-testid="federation-agent-list">
      <div className="federation-map-heading">
        <div><h3>{t('federation.agentMap')}</h3><p>{t('federation.agentMapHint')}</p></div>
        <span>{t('federation.agents', { count: agents.length })}</span>
      </div>
      {selected && <AgentDetail agent={selected.agent}
        {...(selected.sessionId ? { onOpen: () => props.onOpenConversation({
          sessionId: selected.sessionId!, agentId: selected.agent.id,
        }) } : {})} />}
      <div className="federation-agent-grid">
        {agents.map((node) => <button key={nodeKey(node)} className="federation-agent-card"
          aria-pressed={nodeKey(selected) === nodeKey(node)} data-agent-id={node.agent.id}
          data-owner-session-id={node.sessionId}
          data-qualified-agent-id={node.agent.id} data-hub-id={node.agent.hub}
          data-lifecycle={node.agent.lifecycle} onClick={() => setAgentRequest(nodeKey(node))}>
          <span className={`agent-state lifecycle-${node.agent.lifecycle}`} />
          <span><strong>{node.agent.name}</strong><small>{node.agent.id}</small></span>
          <em>{node.agent.hubPath.join(' / ')} · {t(lifecycleKey(node.agent.lifecycle))}</em>
        </button>)}
      </div>
      {agents.length === 0 && <p className="federation-no-agents">{t('federation.noAgents')}</p>}
    </main>
  </div>
}

function HubCard(props: {
  readonly hub: MetaHubInfo
  readonly selected: boolean
  readonly onSelect: () => void
}): React.JSX.Element {
  const { t } = useI18n()
  return <button className="federation-hub" aria-pressed={props.selected}
    data-hub-id={props.hub.name} onClick={props.onSelect}>
    <span className={`agent-state hub-${props.hub.status}`} />
    <strong>{props.hub.name}</strong>
    <small>{t('federation.agents', { count: props.hub.agentCount })}</small>
    <em>{props.hub.capabilities.length
      ? t('federation.capabilities', { value: props.hub.capabilities.join(', ') })
      : t('federation.noCapabilities')}</em>
  </button>
}

function AgentDetail(props: {
  readonly agent: MetaHubTopologyAgent
  readonly onOpen?: () => void
}): React.JSX.Element {
  const { t } = useI18n()
  return <section className="federation-agent-detail" data-testid="federation-agent-detail">
    <div><span className="eyebrow">{t('federation.agentDetails')}</span>
      <h3>{props.agent.name} <small>@ {props.agent.hub}</small></h3></div>
    <dl>
      <div><dt>{t('federation.route')}</dt><dd>{props.agent.hubPath.join(' → ')}</dd></div>
      <div><dt>{t('federation.parent')}</dt><dd>{props.agent.parentId ?? t('federation.root')}</dd></div>
      <div><dt>{t('federation.model')}</dt><dd>{props.agent.model ?? '—'}</dd></div>
      <div><dt>{t('federation.lifecycle')}</dt><dd>{t(lifecycleKey(props.agent.lifecycle))}</dd></div>
    </dl>
    <button disabled={!props.onOpen} onClick={props.onOpen}>
      {t('federation.openConversation')}
    </button>
    {!props.onOpen && <p>{t('federation.conversationUnavailable')}</p>}
    {props.agent.error && <p className="diagnostic-error">{props.agent.error}</p>}
  </section>
}

function nodeKey(node: FederationTopologyNode | undefined): string {
  return node ? `${node.sessionId ?? '__external__'}\u0000${node.agent.id}` : ''
}

const LIFECYCLE_KEYS: Readonly<Record<MetaHubTopologyAgent['lifecycle'], MessageKey>> = {
  spawning: 'federation.lifecycle.spawning', running: 'federation.lifecycle.running',
  finished: 'federation.lifecycle.finished', failed: 'federation.lifecycle.failed',
}
function lifecycleKey(value: MetaHubTopologyAgent['lifecycle']): MessageKey {
  return LIFECYCLE_KEYS[value]
}
