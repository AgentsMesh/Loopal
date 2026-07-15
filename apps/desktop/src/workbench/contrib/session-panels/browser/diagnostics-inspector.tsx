import {
  type AgentControlCommand,
  type HostStatus,
  type SessionDetail,
} from '../../../../shared/contracts'
import { RENDERER_PROTOCOL_VERSION } from '../../../../shared/protocol/renderer-protocol'
import { McpRuntimeSection } from './mcp-runtime-panel'
import { useI18n } from '../../../browser/i18n-context'

export function DiagnosticsInspector(props: {
  readonly hostStatus: HostStatus
  readonly detail: SessionDetail | undefined
  readonly agentId?: string | undefined
  readonly canControl?: boolean | undefined
  readonly busy?: boolean | undefined
  readonly onControl?: ((command: AgentControlCommand) => void) | undefined
}): React.JSX.Element {
  const { locale, t } = useI18n()
  const root = props.detail?.agents.find((agent) => agent.id === props.agentId)
    ?? props.detail?.agents.find((agent) => agent.id === 'main')
    ?? props.detail?.agents[0]
  const telemetry = root?.telemetry
  const view = root?.view ?? (!root?.parentId ? props.detail?.view : undefined)
  const providerError = (root?.conversation ?? props.detail?.conversation ?? [])
    .findLast((entry) => entry.role === 'error')
  return (
    <div className="inspector-content diagnostics" data-testid="diagnostics-pane">
      {view?.hubDegradedSince && (
        <div className="diagnostic-alert">{t('diagnostics.hubDegraded', {
          time: formatTime(view.hubDegradedSince, locale),
        })}</div>
      )}
      {view?.historyTruncated && (
        <div className="diagnostic-alert">{t('diagnostics.historyTruncated')}</div>
      )}
      {root?.error && <div className="diagnostic-alert">{t('diagnostics.agentFailed', { error: root.error })}</div>}
      {providerError && (
        <div className="diagnostic-alert">{t('diagnostics.providerFailed', { error: providerError.text })}</div>
      )}
      <section className="diagnostic-grid">
        <Metric value={props.hostStatus} label={t('diagnostics.desktopHost')} />
        <Metric value={`v${RENDERER_PROTOCOL_VERSION}`} label={t('diagnostics.rendererProtocol')} />
        <Metric value={t('diagnostics.sandboxed')} label={t('diagnostics.rendererSecurity')} />
        <Metric value={root?.status ?? '—'} label={t('diagnostics.selectedAgent')} />
      </section>
      {root && (
        <section className="inspector-section">
          <h3>{t('diagnostics.runtimeConfig')}</h3>
          <Definition label={t('agent.model')} value={root.model} />
          <Definition label={t('agent.mode')} value={root.mode} />
          <Definition label={t('agent.thinking')} value={root.thinkingConfig} />
          <Definition label={t('agent.permission')} value={root.permissionMode} />
          <Definition label={t('agent.decision')} value={root.decisionMode} />
          <Definition label={t('agent.sandbox')} value={root.sandboxPolicy} />
        </section>
      )}
      {telemetry && (
        <section className="inspector-section">
          <h3>{t('diagnostics.usage')}</h3>
          <Definition label={t('diagnostics.turns')} value={String(telemetry.turnCount)} />
          <Definition label={t('diagnostics.inputTokens')} value={telemetry.inputTokens.toLocaleString(locale)} />
          <Definition label={t('diagnostics.outputTokens')} value={telemetry.outputTokens.toLocaleString(locale)} />
          <Definition label={t('diagnostics.cacheRead')} value={telemetry.cacheReadTokens.toLocaleString(locale)} />
          <Definition label={t('diagnostics.thinkingTokens')} value={telemetry.thinkingTokens.toLocaleString(locale)} />
          <Definition label={t('diagnostics.tools')} value={t('diagnostics.toolsValue', {
            active: telemetry.toolsInFlight, total: telemetry.toolCount,
          })} />
        </section>
      )}
      {view && <McpRuntimeSection view={view} canControl={props.canControl} busy={props.busy} onControl={props.onControl} />}
    </div>
  )
}

function Metric({ value, label }: { readonly value: string; readonly label: string }) {
  return <div className="metric"><strong>{value}</strong><span>{label}</span></div>
}

function Definition({ label, value }: {
  readonly label: string
  readonly value: string | undefined
}) {
  if (!value) return null
  return <div className="diagnostic-row"><span>{label}</span><strong>{value}</strong></div>
}

function formatTime(value: string, locale: string): string {
  return new Date(value).toLocaleString(locale)
}
