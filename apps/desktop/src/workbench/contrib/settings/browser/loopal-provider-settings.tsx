import {
  type LoopalBuiltInProviders,
  type LoopalProviderUpdate,
  type LoopalProviderUpdates,
} from '../../../../shared/contracts'
import { useI18n } from '../../../browser/i18n-context'

type ProviderName = keyof LoopalBuiltInProviders

interface Props {
  readonly providers: LoopalBuiltInProviders
  readonly updates: LoopalProviderUpdates
  readonly disabled: boolean
  readonly onChange: (name: ProviderName, update?: LoopalProviderUpdate) => void
}

const labels: Record<ProviderName, string> = {
  anthropic: 'Anthropic', openai: 'OpenAI', google: 'Google',
}

export function LoopalProviderSettings(props: Props): React.JSX.Element {
  const { t } = useI18n()
  return <div className="provider-settings" data-testid="provider-settings">
    <h4>{t('settings.loopal.providers.title')}</h4>
    <p className="muted">{t('settings.loopal.providers.help')}</p>
    {(Object.keys(labels) as ProviderName[]).map((name) => {
      const update = props.updates[name]
      return <ProviderCard key={name} name={name} current={props.providers[name]}
        {...(update ? { update } : {})}
        disabled={props.disabled} onChange={(next) => props.onChange(name, next)} />
    })}
  </div>
}

function ProviderCard(props: {
  name: ProviderName
  current: LoopalBuiltInProviders[ProviderName]
  update?: LoopalProviderUpdate
  disabled: boolean
  onChange(update?: LoopalProviderUpdate): void
}): React.JSX.Element {
  const { t } = useI18n()
  const update = props.update
  const removed = update?.remove === true
  const enabled = removed ? false : update?.enabled ?? props.current.enabled
  const baseUrl = update?.baseUrl ?? props.current.baseUrl
  const apiKeyEnv = update?.apiKeyEnv ?? props.current.apiKeyEnv
  const keyConfigured = update?.apiKey !== undefined
    || (props.current.apiKeyConfigured && !update?.clearApiKey)
  const change = (patch: LoopalProviderUpdate): void => props.onChange({ ...update, ...patch })
  const label = labels[props.name]
  return <fieldset className="provider-card" data-testid={`provider-${props.name}`}>
    <legend>{label}</legend>
    <label className="settings-check"><input type="checkbox"
      aria-label={t('settings.loopal.providers.enable', { provider: label })}
      checked={enabled} disabled={props.disabled || removed}
      onChange={(event) => props.onChange({ enabled: event.currentTarget.checked })} />
      <span>{t(enabled
        ? 'settings.loopal.providers.enabled'
        : 'settings.loopal.providers.disabled')}</span></label>
    <label className="settings-field">
      <span>{t('settings.loopal.providers.baseUrl', { provider: label })}</span><input
      aria-label={t('settings.loopal.providers.baseUrl', { provider: label })}
      value={baseUrl} disabled={props.disabled || !enabled}
      placeholder={t('settings.loopal.providers.default')} onChange={(event) => change({
        enabled: true, baseUrl: event.currentTarget.value,
      })} /></label>
    <label className="settings-field">
      <span>{t('settings.loopal.providers.apiKeyEnv', { provider: label })}</span><input
      aria-label={t('settings.loopal.providers.apiKeyEnv', { provider: label })} value={apiKeyEnv}
      disabled={props.disabled || !enabled} placeholder={`${props.name.toUpperCase()}_API_KEY`}
      onChange={(event) => change({ enabled: true, apiKeyEnv: event.currentTarget.value })} />
    </label>
    <label className="settings-field">
      <span>{t('settings.loopal.providers.apiKey', { provider: label })}</span><input type="password"
      autoComplete="new-password"
      aria-label={t('settings.loopal.providers.apiKey', { provider: label })}
      value={update?.apiKey ?? ''} disabled={props.disabled || !enabled}
      placeholder={t(keyConfigured
        ? 'settings.loopal.providers.configuredValue'
        : 'settings.loopal.providers.notConfigured')}
      onChange={(event) => change({
        enabled: true, apiKey: event.currentTarget.value || undefined, clearApiKey: false,
      })} /></label>
    <div className="settings-actions">
      <button type="button" disabled={props.disabled || !enabled || !keyConfigured}
        onClick={() => change({ apiKey: undefined, clearApiKey: true })}>
        {t('settings.loopal.providers.clearKey')}
      </button>
      <button type="button" disabled={props.disabled}
        onClick={() => props.onChange(removed ? undefined : { remove: true })}>
        {t(removed
          ? 'settings.loopal.providers.undoRemove'
          : 'settings.loopal.providers.removeOverride')}
      </button>
    </div>
  </fieldset>
}
