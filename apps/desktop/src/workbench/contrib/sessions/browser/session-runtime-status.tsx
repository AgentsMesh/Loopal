import { type AgentSummary, type SessionDetail } from '../../../../shared/contracts'
import { useI18n } from '../../../browser/i18n-context'

export function SessionRuntimeStatus(props: {
  readonly detail: SessionDetail
  readonly agentId?: string
}): React.JSX.Element {
  const { locale, t } = useI18n()
  const { detail } = props
  const agent = detail.agents.find((candidate) => candidate.id === props.agentId)
    ?? detail.agents.find((candidate) => candidate.id === 'main') ?? detail.agents[0]
  const telemetry = agent?.telemetry
  const view = agent?.view ?? (!agent?.parentId ? detail.view : undefined)
  const used = telemetry
    ? telemetry.inputTokens + telemetry.outputTokens
      + telemetry.cacheCreationTokens + telemetry.cacheReadTokens
    : 0
  const contextPercent = telemetry?.contextWindow
    ? Math.min(100, Math.round((used / telemetry.contextWindow) * 100))
    : 0
  return (
    <div className="composer-runtime" data-testid="runtime-status" aria-live="polite">
      <span className={`runtime-indicator agent-${agent?.status ?? 'idle'}`} aria-hidden="true" />
      <strong>{statusLabel(agent, detail, view, t)}</strong>
      {agent?.lastTool && <span className="runtime-current-tool">{agent.lastTool}</span>}
      {telemetry?.contextWindow ? (
        <span title={t('runtime.tokens', {
          used: used.toLocaleString(locale),
          total: telemetry.contextWindow.toLocaleString(locale),
        })}>
          {t('runtime.context', { percent: contextPercent })}
        </span>
      ) : null}
    </div>
  )
}

function statusLabel(
  agent: AgentSummary | undefined,
  detail: SessionDetail,
  view: AgentSummary['view'],
  t: ReturnType<typeof useI18n>['t'],
): string {
  if (!agent) return detail.session.status
  const ownsRootView = !agent?.parentId
  const liveEntry = agent?.conversation?.findLast((entry) => entry.streaming)
  if (liveEntry?.role === 'thinking') return t('runtime.thinking')
  if (liveEntry?.role === 'assistant') return t('runtime.streaming')
  if ((ownsRootView || agent.view) && view?.thinkingActive) return t('runtime.thinking')
  if ((ownsRootView || agent.view) && view?.compactBanner) return t('runtime.compacting')
  if ((ownsRootView || agent.view) && view?.streamingText) return t('runtime.streaming')
  if (detail.session.attention === 'permission') return t('runtime.waitPermission')
  if (detail.session.attention === 'question') return t('runtime.waitAnswer')
  if (detail.session.attention === 'plan') return t('runtime.waitPlan')
  if (detail.session.attention === 'failure') return t('runtime.failed')
  if (agent.status === 'running') {
    if ((agent.telemetry?.toolsInFlight ?? 0) > 0) return t('runtime.working')
    const activeAgents = detail.agents.filter((candidate) => (
      candidate.id !== agent.id && ['starting', 'running'].includes(candidate.status)
    )).length
    return activeAgents > 0
      ? t(activeAgents === 1 ? 'runtime.agentWorking' : 'runtime.agentsWorking', {
          count: activeAgents,
        })
      : t('runtime.working')
  }
  if (agent.status === 'waiting') return t('runtime.ready')
  if (agent.status === 'starting') return t('runtime.starting')
  if (agent.status === 'suspended') return t('runtime.suspended')
  if (agent.status === 'completed') return t('runtime.completed')
  if (agent.status === 'failed') return t('runtime.failed')
  return t('runtime.idle')
}
