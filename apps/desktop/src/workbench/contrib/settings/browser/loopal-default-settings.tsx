import { useEffect, useState } from 'react'
import {
  UpdateLoopalSettingsInputSchema,
  type LoopalBuiltInProviders,
  type LoopalDefaultSettings,
  type LoopalDesktopAPI,
  type LoopalProviderUpdate,
  type LoopalProviderUpdates,
  type LoopalSettingsValues,
} from '../../../../shared/contracts'
import { LoopalAdvancedSettings } from './loopal-advanced-settings'
import { LoopalCompatibleProviderSettings } from './loopal-compatible-provider-settings'
import { LoopalProviderSettings } from './loopal-provider-settings'
import { LoopalSettingsForm } from './loopal-settings-form'
import { useI18n } from '../../../browser/i18n-context'

interface Props {
  readonly api: LoopalDesktopAPI
  readonly workspaceId?: string
  readonly sessionId?: string
  readonly section?: 'all' | 'defaults' | 'providers' | 'hidden'
}

export function LoopalDefaultSettings(props: Props): React.JSX.Element | null {
  const { t } = useI18n()
  const [record, setRecord] = useState<LoopalDefaultSettings>()
  const [draft, setDraft] = useState<LoopalSettingsValues>()
  const [providerUpdates, setProviderUpdates] = useState<LoopalProviderUpdates>({})
  const [busy, setBusy] = useState(false)
  const [message, setMessage] = useState<string>()
  const [error, setError] = useState<string>()
  useEffect(() => {
    let live = true
    setRecord(undefined)
    setDraft(undefined)
    setProviderUpdates({})
    setMessage(undefined)
    setError(undefined)
    if (!props.workspaceId) return () => { live = false }
    void props.api.getLoopalSettings(props.workspaceId).then((next) => {
      if (!live) return
      setRecord(next)
      setDraft(next.settings)
    }, (reason) => {
      if (live) setError(errorText(reason))
    })
    return () => { live = false }
  }, [props.api, props.workspaceId])

  const save = async (
    current: LoopalDefaultSettings,
    settingsDraft: LoopalSettingsValues,
    providers: LoopalProviderUpdates,
  ): Promise<void> => {
    setBusy(true)
    setError(undefined)
    setMessage(undefined)
    try {
      const input = UpdateLoopalSettingsInputSchema.parse({
        workspaceId: current.workspaceId,
        settings: settingsDraft,
        ...(Object.keys(providers).length ? { providerUpdates: providers } : {}),
      })
      const next = await props.api.updateLoopalSettings(input)
      setRecord(next)
      setDraft(next.settings)
      setProviderUpdates({})
      setMessage(t('settings.loopal.defaults.saved'))
    } catch (reason) {
      setError(errorText(reason))
    } finally {
      setBusy(false)
    }
  }
  const restart = async (sessionId: string): Promise<void> => {
    setBusy(true)
    setError(undefined)
    try {
      await props.api.restartSession(sessionId)
      setMessage(t('settings.loopal.defaults.restarted'))
    } catch (reason) {
      setError(errorText(reason))
    } finally {
      setBusy(false)
    }
  }
  const changed = record && draft
    ? JSON.stringify(record.settings) !== JSON.stringify(draft)
      || Object.keys(providerUpdates).length > 0
    : false
  const showDefaults = props.section !== 'providers'
    && props.section !== 'hidden'
  const showProviders = props.section !== 'defaults' && props.section !== 'hidden'
  const updateProvider = (
    name: keyof LoopalBuiltInProviders,
    update?: LoopalProviderUpdate,
  ): void => setProviderUpdates((current) => {
    const next = { ...current }
    if (update) next[name] = update
    else delete next[name]
    return next
  })
  const updateCompatible = (
    updates: NonNullable<LoopalProviderUpdates['openaiCompatible']>,
  ): void => setProviderUpdates((current) => {
    const next = { ...current }
    if (updates.length) next.openaiCompatible = updates
    else delete next.openaiCompatible
    return next
  })
  if (props.section === 'hidden') return null
  return <section className="settings-section" data-testid="loopal-default-settings">
    <h3>{t(showProviders && !showDefaults
      ? 'settings.loopal.providers.sectionTitle'
      : 'settings.loopal.defaults.title')}</h3>
    <p className="muted">{t('settings.loopal.defaults.storage')}</p>
    {!props.workspaceId && <p className="muted">{t('settings.loopal.defaults.openWorkspace')}</p>}
    {props.workspaceId && !draft && !error && <p className="muted">
      {t('settings.loopal.defaults.loading')}
    </p>}
    {draft && showDefaults && <LoopalSettingsForm value={draft} disabled={busy}
      onChange={(patch) => setDraft({ ...draft, ...patch })} />}
    {record && showProviders && <LoopalProviderSettings providers={record.providers}
      updates={providerUpdates} disabled={busy} onChange={updateProvider} />}
    {record && showProviders && <LoopalCompatibleProviderSettings providers={record.openaiCompatible ?? []}
      updates={providerUpdates.openaiCompatible ?? []} disabled={busy}
      onChange={updateCompatible} />}
    {record && showProviders && <p className="muted" data-testid="configured-providers">
      {t('settings.loopal.providers.configured', {
        providers: record.configuredProviders.join(', ') || t('settings.loopal.providers.none'),
      })}
    </p>}
    {record && showDefaults && <LoopalAdvancedSettings record={record} />}
    {error && <p role="alert">{error}</p>}
    {message && <p role="status">{message}</p>}
    {record && draft && <div className="settings-actions">
      <button disabled={busy || !changed}
        onClick={() => void save(record, draft, providerUpdates)}>
        {t(showProviders && !showDefaults
          ? 'settings.loopal.providers.save'
          : 'settings.loopal.defaults.save')}
      </button>
      {props.sessionId && <button disabled={busy || changed}
        onClick={() => void restart(props.sessionId!)}>
        {t('settings.loopal.defaults.restart')}
      </button>}
    </div>}
  </section>
}

function errorText(value: unknown): string {
  return value instanceof Error ? value.message : String(value)
}
