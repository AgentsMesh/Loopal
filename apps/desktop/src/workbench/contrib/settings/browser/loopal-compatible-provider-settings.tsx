import {
  type LoopalOpenAiCompatibleSettings,
  type LoopalOpenAiCompatibleUpdate,
} from '../../../../shared/contracts'
import { useI18n } from '../../../browser/i18n-context'

interface Props {
  readonly providers: readonly LoopalOpenAiCompatibleSettings[]
  readonly updates: readonly LoopalOpenAiCompatibleUpdate[]
  readonly disabled: boolean
  readonly onChange: (updates: LoopalOpenAiCompatibleUpdate[]) => void
}

export function LoopalCompatibleProviderSettings(props: Props): React.JSX.Element {
  const { t } = useI18n()
  const newUpdates = props.updates.filter((update) =>
    !props.providers.some((provider) => provider.name === update.name) && !update.remove)
  const cards = [
    ...props.providers.map((provider) => ({ provider, isNew: false })),
    ...newUpdates.map((update) => ({ provider: emptyProvider(update.name), isNew: true })),
  ]
  const replace = (name: string, update?: LoopalOpenAiCompatibleUpdate): void => {
    const next = props.updates.filter((item) => item.name !== name)
    if (update) next.push(update)
    props.onChange(next)
  }
  const add = (): void => {
    const used = new Set(cards.map(({ provider }) => provider.name))
    let index = 1
    while (used.has(`compatible-${index}`)) index += 1
    props.onChange([...props.updates, { name: `compatible-${index}`, baseUrl: '' }])
  }
  return <div className="provider-settings" data-testid="compatible-provider-settings">
    <div className="settings-actions">
      <h4>{t('settings.loopal.compatible.title')}</h4>
      <button type="button" disabled={props.disabled} onClick={add}>
        {t('settings.loopal.compatible.add')}
      </button>
    </div>
    <p className="muted">{t('settings.loopal.compatible.help')}</p>
    {cards.map(({ provider, isNew }) => {
      const update = props.updates.find((item) => item.name === provider.name)
      return <CompatibleCard key={provider.name} current={provider}
        {...(update ? { update } : {})}
        isNew={isNew} disabled={props.disabled} replace={replace} />
    })}
  </div>
}

function CompatibleCard(props: {
  current: LoopalOpenAiCompatibleSettings
  update?: LoopalOpenAiCompatibleUpdate
  isNew: boolean
  disabled: boolean
  replace(name: string, update?: LoopalOpenAiCompatibleUpdate): void
}): React.JSX.Element {
  const { t } = useI18n()
  const name = props.current.name
  const update = props.update
  const removed = update?.remove === true
  const change = (patch: Partial<LoopalOpenAiCompatibleUpdate>): void =>
    props.replace(name, { name, ...update, ...patch })
  const keyConfigured = update?.apiKey !== undefined
    || (props.current.apiKeyConfigured && !update?.clearApiKey)
  return <fieldset className="provider-card" data-testid="provider-openai-compatible"
    data-provider-name={name}>
    <legend>{name}</legend>
    <label className="settings-field"><span>{t('settings.loopal.compatible.providerName')}</span><input
      aria-label={t('settings.loopal.compatible.providerNameAria')} value={name}
      disabled={props.disabled || !props.isNew || removed}
      onChange={(event) => props.replace(name, {
        ...update, name: event.currentTarget.value, baseUrl: update?.baseUrl ?? '',
      })} /></label>
    <label className="settings-field"><span>{t('settings.loopal.compatible.baseUrl')}</span><input
      aria-label={t('settings.loopal.compatible.baseUrlAria')}
      value={update?.baseUrl ?? props.current.baseUrl}
      disabled={props.disabled || removed} placeholder="https://provider.example/v1"
      onChange={(event) => change({ baseUrl: event.currentTarget.value })} /></label>
    <label className="settings-field"><span>{t('settings.loopal.compatible.modelPrefix')}</span><input
      aria-label={t('settings.loopal.compatible.modelPrefixAria')}
      value={update?.modelPrefix ?? props.current.modelPrefix}
      disabled={props.disabled || removed} placeholder="deepseek-"
      onChange={(event) => change({ modelPrefix: event.currentTarget.value })} /></label>
    <label className="settings-field"><span>{t('settings.loopal.compatible.apiKeyEnv')}</span><input
      aria-label={t('settings.loopal.compatible.apiKeyEnvAria')}
      value={update?.apiKeyEnv ?? props.current.apiKeyEnv} disabled={props.disabled || removed}
      placeholder="COMPAT_API_KEY"
      onChange={(event) => change({ apiKeyEnv: event.currentTarget.value })} /></label>
    <label className="settings-field"><span>{t('settings.loopal.compatible.apiKey')}</span><input type="password"
      autoComplete="new-password" aria-label={t('settings.loopal.compatible.apiKeyAria')}
      value={update?.apiKey ?? ''} disabled={props.disabled || removed}
      placeholder={t(keyConfigured
        ? 'settings.loopal.providers.configuredValue'
        : 'settings.loopal.providers.notConfigured')}
      onChange={(event) => change({
        apiKey: event.currentTarget.value || undefined, clearApiKey: false,
      })} /></label>
    <div className="settings-actions">
      <button type="button" disabled={props.disabled || removed || !keyConfigured}
        onClick={() => change({ apiKey: undefined, clearApiKey: true })}>
        {t('settings.loopal.providers.clearKey')}
      </button>
      <button type="button" disabled={props.disabled} onClick={() => {
        if (props.isNew || removed) props.replace(name)
        else props.replace(name, { name, remove: true })
      }}>{t(removed
          ? 'settings.loopal.providers.undoRemove'
          : 'settings.loopal.compatible.remove')}</button>
    </div>
  </fieldset>
}

function emptyProvider(name: string): LoopalOpenAiCompatibleSettings {
  return { name, baseUrl: '', apiKeyEnv: '', modelPrefix: '', apiKeyConfigured: false }
}
