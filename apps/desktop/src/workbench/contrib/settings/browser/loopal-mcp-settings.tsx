import { useEffect, useState } from 'react'
import {
  type LoopalDesktopAPI, type McpServerDefinition, type McpServersResponse,
} from '../../../../shared/contracts'
import {
  editMcpServerDraft, mcpInputFromDraft, newMcpServerDraft, type McpServerDraft,
} from './mcp-server-draft'
import { McpServerForm } from './mcp-server-form'
import { useI18n } from '../../../browser/i18n-context'
import './mcp-settings.css'

export function LoopalMcpSettings(props: {
  readonly api: LoopalDesktopAPI
  readonly workspaceId?: string
  readonly visible?: boolean
}): React.JSX.Element | null {
  const { t } = useI18n()
  const [record, setRecord] = useState<McpServersResponse>()
  const [draft, setDraft] = useState<McpServerDraft>()
  const [busy, setBusy] = useState(false)
  const [message, setMessage] = useState<string>()
  const [error, setError] = useState<string>()
  useEffect(() => {
    let active = true
    setRecord(undefined); setDraft(undefined); setError(undefined); setMessage(undefined)
    if (!props.workspaceId) return () => { active = false }
    void props.api.listMcpServers(props.workspaceId).then((next) => {
      if (active) setRecord(next)
    }, (reason) => { if (active) setError(errorText(reason)) })
    return () => { active = false }
  }, [props.api, props.workspaceId])
  const execute = async (operation: () => Promise<McpServersResponse>, status: string) => {
    setBusy(true); setError(undefined); setMessage(undefined)
    try {
      const next = await operation()
      setRecord(next); setDraft(undefined); setMessage(status)
    } catch (reason) {
      setError(errorText(reason))
    } finally {
      setBusy(false)
    }
  }
  const save = (): void => {
    if (!record || !draft) return
    void execute(() => props.api.upsertMcpServer({
      workspaceId: record.workspaceId, server: mcpInputFromDraft(draft),
    }), t('settings.mcp.saved'))
  }
  const remove = (server: McpServerDefinition): void => {
    if (!record) return
    void execute(() => props.api.deleteMcpServer({
      workspaceId: record.workspaceId, name: server.name,
    }), t('settings.mcp.deleted', { name: server.name }))
  }
  if (props.visible === false) return null
  return <section className="settings-section loopal-mcp-settings" data-testid="loopal-mcp-settings">
    <div className="mcp-heading">
      <div><h3>{t('settings.mcp.title')}</h3>
        <p className="muted">{t('settings.mcp.help')}</p></div>
      {record && !draft && <button disabled={busy} onClick={() => setDraft(newMcpServerDraft())}>
        {t('settings.mcp.add')}
      </button>}
    </div>
    {!props.workspaceId && <p className="muted">{t('settings.mcp.openWorkspace')}</p>}
    {props.workspaceId && !record && !error && <p className="muted">{t('settings.mcp.loading')}</p>}
    {record && !draft && <div className="mcp-server-list">
      {record.servers.length === 0 && <p className="muted">{t('settings.mcp.empty')}</p>}
      {record.servers.map((server) => <article className="mcp-server-card" key={server.name}>
        <div><strong>{server.name}</strong><small>{server.type} · {server.source}</small></div>
        <span className={server.enabled ? 'enabled' : 'disabled'}>
          {t(server.enabled ? 'settings.mcp.enabled' : 'settings.mcp.disabled')}
        </span>
        <code>{server.type === 'stdio' ? server.command : server.url}</code>
        <div className="mcp-actions">
          <button disabled={busy} onClick={() => setDraft(editMcpServerDraft(server))}>
            {t('settings.mcp.edit')}
          </button>
          <button disabled={busy} aria-label={t('settings.mcp.deleteAria', { name: server.name })}
            onClick={() => remove(server)}>{t('settings.mcp.delete')}</button>
        </div>
      </article>)}
    </div>}
    {draft && <McpServerForm draft={draft} busy={busy} onChange={setDraft}
      onSave={save} onCancel={() => setDraft(undefined)} />}
    {error && <p role="alert" className="diagnostic-error">{error}</p>}
    {message && <p role="status">{message}</p>}
  </section>
}

function errorText(value: unknown): string {
  return value instanceof Error ? value.message : String(value)
}
