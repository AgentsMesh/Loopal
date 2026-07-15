import { useState } from 'react'
import { type McpSecretPatch } from '../../../../shared/contracts'
import { type McpServerDraft, withSecretPatch } from './mcp-server-draft'
import { useI18n } from '../../../browser/i18n-context'

export function McpSecretEditor(props: {
  readonly draft: McpServerDraft
  readonly disabled: boolean
  readonly onChange: (draft: McpServerDraft) => void
}): React.JSX.Element {
  const { t } = useI18n()
  const [name, setName] = useState('')
  const [value, setValue] = useState('')
  const target = props.draft.type === 'stdio' ? 'env' : 'header'
  const label = t(target === 'env' ? 'settings.mcp.secrets.env' : 'settings.mcp.secrets.headers')
  const targetLabel = t(`settings.mcp.secrets.target.${target}`)
  const patch = (next: McpSecretPatch | undefined, key: string): void => {
    props.onChange(withSecretPatch(props.draft, next, key))
  }
  return <fieldset className="mcp-secrets" disabled={props.disabled}>
    <legend>{t('settings.mcp.secrets.legend', { label })}</legend>
    <p className="muted">{t('settings.mcp.secrets.help')}</p>
    {props.draft.secrets.map((secret) => {
      const pending = props.draft.secretPatches.find((candidate) => candidate.name === secret.name)
      return <div className="mcp-secret-row" key={secret.name}>
        <span><code>{secret.name}</code><small>{pending
          ? t(`settings.mcp.secrets.pending.${pending.operation}`)
          : t(secret.configured
            ? 'settings.mcp.secrets.configured'
            : 'settings.mcp.secrets.notConfigured')}</small></span>
        <input type="password" autoComplete="off" value={pending?.operation === 'set'
          ? pending.value : ''} aria-label={t('settings.mcp.secrets.value', { name: secret.name })}
          placeholder={t('settings.mcp.secrets.keep')}
          onChange={(event) => patch(event.target.value ? {
            target, name: secret.name, operation: 'set', value: event.target.value,
          } : undefined, secret.name)} />
        <button type="button" aria-label={t('settings.mcp.secrets.remove', { name: secret.name })}
          onClick={() => patch(pending?.operation === 'remove' ? undefined : {
            target, name: secret.name, operation: 'remove',
          }, secret.name)}>{t(pending?.operation === 'remove'
            ? 'settings.mcp.secrets.undo'
            : 'settings.mcp.secrets.removeButton')}</button>
      </div>
    })}
    {props.draft.secretPatches.filter((candidate) => candidate.operation === 'set'
      && !props.draft.secrets.some((secret) => secret.name === candidate.name))
      .map((candidate) => <div className="mcp-secret-row" key={candidate.name}>
        <span><code>{candidate.name}</code><small>{t('settings.mcp.secrets.willConfigure')}</small></span>
        <input type="password" autoComplete="off"
          aria-label={t('settings.mcp.secrets.value', { name: candidate.name })}
          value={candidate.operation === 'set' ? candidate.value : ''}
          onChange={(event) => patch(event.target.value ? {
            ...candidate, operation: 'set', value: event.target.value,
          } : undefined, candidate.name)} />
        <button type="button" onClick={() => patch(undefined, candidate.name)}>
          {t('common.cancel')}
        </button>
      </div>)}
    <div className="mcp-secret-add">
      <input aria-label={t('settings.mcp.secrets.newName', { target: targetLabel })} value={name}
        placeholder={t('settings.mcp.secrets.namePlaceholder')}
        onChange={(event) => setName(event.target.value)} />
      <input aria-label={t('settings.mcp.secrets.newValue', { target: targetLabel })} type="password"
        autoComplete="off" value={value} placeholder={t('settings.mcp.secrets.valuePlaceholder')}
        onChange={(event) => setValue(event.target.value)} />
      <button type="button" disabled={!name || !value} onClick={() => {
        patch({ target, name, operation: 'set', value }, name); setName(''); setValue('')
      }}>{t('settings.mcp.secrets.set')}</button>
    </div>
  </fieldset>
}
