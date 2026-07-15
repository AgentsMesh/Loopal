import { type AgentControlCommand, type SessionView } from '../../../../shared/contracts'
import { useI18n } from '../../../browser/i18n-context'

interface McpRuntimeProps {
  readonly view: SessionView
  readonly canControl?: boolean | undefined
  readonly busy?: boolean | undefined
  readonly onControl?: ((command: AgentControlCommand) => void) | undefined
}

export function McpRuntimePanel(props: McpRuntimeProps): React.JSX.Element {
  return (
    <div className="inspector-content diagnostics" data-testid="mcp-runtime-pane">
      <McpRuntimeSection {...props} />
    </div>
  )
}

export function McpRuntimeSection(props: McpRuntimeProps): React.JSX.Element {
  const { t } = useI18n()
  return (
    <section className="inspector-section">
      <div className="inspector-section-heading">
        <h3>{t('settings.mcp.runtime.title', { count: props.view.mcpServers.length })}</h3>
        {props.onControl && (
          <button
            disabled={!props.canControl || props.busy}
            aria-label={t('settings.mcp.runtime.refresh')}
            onClick={() => props.onControl?.({ type: 'mcp_status' })}
          >{t('settings.mcp.runtime.refreshButton')}</button>
        )}
      </div>
      {props.view.mcpServers.map((server) => (
        <details className="mcp-row" key={server.name}>
          <summary><strong>{server.name}</strong><small>{server.status}</small></summary>
          <p>{server.transport} · {server.source}</p>
          <small>
            {t('settings.mcp.runtime.tools', { count: server.toolCount })} ·
            {' '}{t('settings.mcp.runtime.resources', { count: server.resourceCount })} ·
            {' '}{t('settings.mcp.runtime.prompts', { count: server.promptCount })}
          </small>
          {server.errors.map((error) => (
            <p className="diagnostic-error" key={error}>{error}</p>
          ))}
          {props.onControl && (
            <div className="resource-actions">
              {isConnected(server.status) ? (
                <button
                  disabled={!props.canControl || props.busy}
                  aria-label={t('settings.mcp.runtime.disconnect', { name: server.name })}
                  onClick={() => props.onControl?.({
                    type: 'mcp_disconnect', server: server.name,
                  })}
                >{t('settings.mcp.runtime.disconnectButton')}</button>
              ) : (
                <button
                  disabled={!props.canControl || props.busy}
                  aria-label={t('settings.mcp.runtime.reconnect', { name: server.name })}
                  onClick={() => props.onControl?.({
                    type: 'mcp_reconnect', server: server.name,
                  })}
                >{t('settings.mcp.runtime.reconnectButton')}</button>
              )}
            </div>
          )}
        </details>
      ))}
      {props.view.mcpServers.length === 0 && (
        <p className="muted">{t('settings.mcp.runtime.empty')}</p>
      )}
    </section>
  )
}

function isConnected(status: string): boolean {
  return ['ready', 'connected', 'running'].includes(status.toLocaleLowerCase())
}
