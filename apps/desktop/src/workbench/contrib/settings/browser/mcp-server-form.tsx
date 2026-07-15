import { type McpServerDraft } from './mcp-server-draft'
import { McpSecretEditor } from './mcp-secret-editor'
import { useI18n } from '../../../browser/i18n-context'

export function McpServerForm(props: {
  readonly draft: McpServerDraft
  readonly busy: boolean
  readonly onChange: (draft: McpServerDraft) => void
  readonly onSave: () => void
  readonly onCancel: () => void
}): React.JSX.Element {
  const { t } = useI18n()
  const update = (patch: Partial<McpServerDraft>): void => props.onChange({ ...props.draft, ...patch })
  return <form className="mcp-server-form" onSubmit={(event) => {
    event.preventDefault(); props.onSave()
  }}>
    <div className="mcp-form-grid">
      <Field label={t('settings.mcp.transport')}>
        <select aria-label={t('settings.mcp.transport')} value={props.draft.type} disabled={props.busy}
          onChange={(event) => update({
            type: event.target.value as McpServerDraft['type'], secrets: [], secretPatches: [],
          })}>
          <option value="stdio">stdio</option>
          <option value="streamable-http">streamable-http</option>
        </select>
      </Field>
      <Field label={t('settings.mcp.serverName')}>
        <input aria-label={t('settings.mcp.serverName')} value={props.draft.name}
          disabled={props.busy || props.draft.lockedName} maxLength={64}
          onChange={(event) => update({ name: event.target.value })} />
      </Field>
      <Field label={t('settings.mcp.sharing')}>
        <select aria-label={t('settings.mcp.sharing')} value={props.draft.sharing} disabled={props.busy}
          onChange={(event) => update({ sharing: event.target.value as McpServerDraft['sharing'] })}>
          <option value="hub-singleton">{t('settings.mcp.sharing.singleton')}</option>
          <option value="per-agent">{t('settings.mcp.sharing.agent')}</option>
          <option value="spawn-tree">{t('settings.mcp.sharing.tree')}</option>
        </select>
      </Field>
      <Field label={t('settings.mcp.timeout')}>
        <input aria-label={t('settings.mcp.timeout')} type="number" min={100} max={600_000}
          value={props.draft.timeoutMs} disabled={props.busy}
          onChange={(event) => update({ timeoutMs: Number(event.target.value) })} />
      </Field>
    </div>
    <label className="settings-check"><input aria-label={t('settings.mcp.enable')} type="checkbox"
      checked={props.draft.enabled} disabled={props.busy}
      onChange={(event) => update({ enabled: event.target.checked })} />
      <span>{t('settings.mcp.enableHint')}</span></label>
    {props.draft.restrictedSecrets && <p className="mcp-warning" role="note">
      {t('settings.mcp.restrictedSecrets')}
    </p>}
    {props.draft.type === 'stdio' ? <>
      <div className="mcp-form-grid">
        <Field label={t('settings.mcp.command')}><input aria-label={t('settings.mcp.command')}
          value={props.draft.command}
          disabled={props.busy} maxLength={1024}
          onChange={(event) => update({ command: event.target.value })} /></Field>
        <Field label={t('settings.mcp.arguments')}>
          <textarea aria-label={t('settings.mcp.argumentsAria')}
          value={props.draft.argsText} disabled={props.busy}
          onChange={(event) => update({ argsText: event.target.value })} />
        </Field>
      </div>
      <p className="mcp-warning">{t('settings.mcp.argumentsHelp')}</p>
      <label className="settings-check"><input aria-label={t('settings.mcp.cwdIsolation')}
        type="checkbox"
        checked={props.draft.cwdIsolation} disabled={props.busy}
        onChange={(event) => update({ cwdIsolation: event.target.checked })} />
        <span>{t('settings.mcp.cwdIsolationHint')}</span></label>
      {props.draft.cwdIsolation && <div className="mcp-form-grid">
        <Field label={t('settings.mcp.cwdArgument')}>
          <input aria-label={t('settings.mcp.cwdArgument')}
          value={props.draft.cwdArg} disabled={props.busy}
          onChange={(event) => update({ cwdArg: event.target.value })} />
        </Field>
        <Field label={t('settings.mcp.cwdCache')}><input
          aria-label={t('settings.mcp.cwdCache')} value={props.draft.cacheSubdir}
          disabled={props.busy}
          onChange={(event) => update({ cacheSubdir: event.target.value })} /></Field>
      </div>}
    </> : <>
      <Field label={t('settings.mcp.httpUrl')}><input aria-label={t('settings.mcp.httpUrl')}
        value={props.draft.url}
        disabled={props.busy} maxLength={2048} placeholder="https://mcp.example.test/api"
        onChange={(event) => update({ url: event.target.value })} /></Field>
      <p className="mcp-warning">{t('settings.mcp.httpHelp')}</p>
    </>}
    <McpSecretEditor draft={props.draft} disabled={props.busy} onChange={props.onChange} />
    <div className="mcp-actions">
      <button type="submit" disabled={props.busy}>{t('settings.mcp.save')}</button>
      <button type="button" disabled={props.busy} onClick={props.onCancel}>
        {t('common.cancel')}
      </button>
    </div>
  </form>
}

function Field(props: {
  readonly label: string
  readonly children: React.ReactNode
}): React.JSX.Element {
  return <label className="settings-field"><span>{props.label}</span>{props.children}</label>
}
