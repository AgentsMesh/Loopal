import { useEffect, useState } from 'react'
import {
  type LocalMetaHubStatus,
  type MetaHubRuntimeState,
  type MetaHubRuntimeTarget,
  type MetaHubSettings,
  type LoopalDesktopAPI,
  UpdateMetaHubSettingsInputSchema,
} from '../../../../shared/contracts'
import { federationHubName } from '../../../../shared/contracts/metahub-identity'
import { MetaHubTopologyPanel } from '../../federation/browser/metahub-topology-panel'
import { useI18n, type I18nContextValue } from '../../../browser/i18n-context'
import './metahub-settings.css'

const emptySettings: MetaHubSettings = {
  address: '', hubName: 'loopal-desktop', joinOnStart: false,
  startLocalOnLaunch: false, tokenConfigured: false,
}

export function MetaHubSettingsSection(props: {
  readonly api: LoopalDesktopAPI
  readonly target?: MetaHubRuntimeTarget
  readonly initialState?: MetaHubRuntimeState
  readonly visible?: boolean
}): React.JSX.Element | null {
  const { t } = useI18n()
  const [settings, setSettings] = useState(emptySettings)
  // reason: until the persisted settings arrive, `settings` holds placeholder
  // defaults — editing or saving in that window would overwrite the user's
  // stored values (and join under the placeholder hub name), so the form and
  // every settings-writing action stay disabled while !loaded.
  const [loaded, setLoaded] = useState(false)
  const [token, setToken] = useState('')
  const [runtime, setRuntime] = useState(props.initialState)
  const [local, setLocal] = useState<LocalMetaHubStatus>({ state: 'stopped' })
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string>()
  useEffect(() => {
    let active = true
    const acceptError = (value: unknown): void => { if (active) setError(message(value)) }
    void props.api.getMetaHubSettings().then((next) => {
      if (active) { setSettings(next); setLoaded(true) }
    }).catch(acceptError)
    void props.api.getLocalMetaHubStatus().then((next) => {
      if (active) setLocal(next)
    }).catch(acceptError)
    if (props.target) void props.api.getMetaHubStatus(props.target).then((next) => {
      if (active) setRuntime(next)
    }).catch(acceptError)
    return () => { active = false }
  }, [props.target?.sessionId, props.target?.runtimeId, props.target?.generation])

  const execute = async (operation: () => Promise<void>): Promise<void> => {
    setBusy(true); setError(undefined)
    try { await operation() }
    catch (value) { setError(message(value)) }
    finally { setBusy(false) }
  }
  const save = async (): Promise<MetaHubSettings> => {
    const input = UpdateMetaHubSettingsInputSchema.parse({
      address: settings.address,
      hubName: settings.hubName,
      joinOnStart: settings.joinOnStart,
      startLocalOnLaunch: settings.startLocalOnLaunch,
      ...(token ? { token } : {}),
    })
    const next = await props.api.updateMetaHubSettings(input)
    setSettings(next); setToken('')
    return next
  }
  const join = (): void => void execute(async () => {
    if (!props.target) throw new Error(t('settings.metahub.error.join'))
    const next = await save()
    setRuntime(await props.api.joinMetaHub({
      ...props.target, hubName: federationHubName(next.hubName, props.target),
    }))
  })
  const refresh = (): void => void execute(async () => {
    if (!props.target) throw new Error(t('settings.metahub.error.refresh'))
    setRuntime(await props.api.getMetaHubStatus(props.target))
  })
  const disconnect = (): void => void execute(async () => {
    if (!props.target) throw new Error(t('settings.metahub.error.disconnect'))
    setRuntime(await props.api.disconnectMetaHub(props.target))
  })
  const startLocal = (): void => void execute(async () => {
    setLocal(await props.api.startLocalMetaHub({ bindAddress: '127.0.0.1:0' }))
    const next = await props.api.getMetaHubSettings()
    setSettings(next)
    if (props.target) setRuntime(await props.api.joinMetaHub({
      ...props.target, hubName: federationHubName(next.hubName, props.target),
    }))
  })
  const stopLocal = (): void => void execute(async () => {
    if (runtime?.state === 'connected' && runtime.address === local.address && props.target) {
      setRuntime(await props.api.disconnectMetaHub(props.target))
    }
    setLocal(await props.api.stopLocalMetaHub())
  })

  if (props.visible === false) return null
  return (
    <section className="settings-section metahub-settings" data-testid="metahub-settings">
      <div className="metahub-heading">
        <div><h3>{t('settings.metahub.title')}</h3>
          <small>{statusLabel(runtime, local, t)}</small></div>
        <button disabled={busy || !props.target} onClick={refresh}>
          {t('settings.metahub.refresh')}
        </button>
      </div>
      <div className="metahub-fields">
        <Field label={t('settings.metahub.address')}>
          <input aria-label={t('settings.metahub.addressAria')} value={settings.address}
            disabled={!loaded}
            onChange={(event) => setSettings({ ...settings, address: event.target.value })} />
        </Field>
        <Field label={t('settings.metahub.hubName')}>
          <input aria-label={t('settings.metahub.hubNameAria')} value={settings.hubName}
            disabled={!loaded}
            onChange={(event) => setSettings({ ...settings, hubName: event.target.value })} />
        </Field>
        <Field label={t('settings.metahub.token')}>
          <input aria-label={t('settings.metahub.tokenAria')} type="password" value={token}
            autoComplete="off" disabled={!loaded} placeholder={t(settings.tokenConfigured
              ? 'settings.metahub.tokenConfigured'
              : 'settings.metahub.tokenRequired')}
            onChange={(event) => setToken(event.target.value)} />
        </Field>
      </div>
      <label className="settings-check"><input aria-label={t('settings.metahub.joinOnStartAria')}
        type="checkbox" checked={settings.joinOnStart} disabled={!loaded}
        onChange={(event) => setSettings({ ...settings, joinOnStart: event.target.checked })} />
        <span>{t('settings.metahub.joinOnStart')}</span></label>
      <label className="settings-check"><input aria-label={t('settings.metahub.startLocalAria')}
        type="checkbox" checked={settings.startLocalOnLaunch} disabled={!loaded}
        onChange={(event) => setSettings({ ...settings, startLocalOnLaunch: event.target.checked })} />
        <span>{t('settings.metahub.startLocal')}</span></label>
      <div className="metahub-actions">
        <button disabled={busy || !loaded} onClick={() => void execute(async () => { await save() })}>
          {t('common.save')}
        </button>
        <button disabled={busy || !loaded || !settings.tokenConfigured}
          onClick={() => void execute(async () => {
            const next = UpdateMetaHubSettingsInputSchema.parse({
              ...settings, clearToken: true,
            })
            setSettings(await props.api.updateMetaHubSettings(next)); setToken('')
          })}>{t('settings.metahub.clearToken')}</button>
        <button disabled={busy || !loaded || !props.target} onClick={join}>
          {t('settings.metahub.join')}
        </button>
        <button disabled={busy || !props.target || runtime?.state === 'disconnected'}
          onClick={disconnect}>{t('settings.metahub.disconnect')}</button>
        {local.state !== 'stopped' && <button disabled={busy} onClick={stopLocal}>
          {t(local.state === 'failed'
            ? 'settings.metahub.clearFailed'
            : 'settings.metahub.stopLocal')}
        </button>}
        {local.state !== 'running' && <button disabled={busy} onClick={startLocal}>
          {t(local.state === 'failed'
            ? 'settings.metahub.restartLocal'
            : 'settings.metahub.startLocalAndJoin')}
        </button>}
      </div>
      {error && <p className="diagnostic-error" role="alert">{error}</p>}
      <MetaHubTopologyPanel state={runtime} />
    </section>
  )
}

function Field(props: { readonly label: string; readonly children: React.ReactNode }): React.JSX.Element {
  return <label className="settings-field"><span>{props.label}</span>{props.children}</label>
}
function statusLabel(
  runtime: MetaHubRuntimeState | undefined,
  local: LocalMetaHubStatus,
  t: I18nContextValue['t'],
): string {
  const cluster = runtime
    ? t(`settings.metahub.status.${runtime.state}`)
    : t('settings.metahub.status.noSession')
  return t('settings.metahub.status.summary', {
    cluster, local: t(`settings.metahub.status.${local.state}`),
  })
}
function message(value: unknown): string { return value instanceof Error ? value.message : String(value) }
