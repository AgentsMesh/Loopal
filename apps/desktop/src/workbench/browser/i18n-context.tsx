import {
  createContext, useCallback, useContext, useEffect, useMemo, useState,
  type ReactNode,
} from 'react'
import {
  DEFAULT_DESKTOP_PREFERENCES,
  type DesktopLocalePreference,
  type DesktopPreferences,
  type LoopalDesktopAPI,
} from '../../shared/contracts'
import {
  resolveLocale, translate, type MessageKey, type SupportedLocale,
} from '../../shared/i18n'

type TranslationValues = Readonly<Record<string, string | number>>

export interface I18nContextValue {
  readonly locale: SupportedLocale
  readonly preference: DesktopLocalePreference
  readonly ready: boolean
  readonly t: (key: MessageKey, values?: TranslationValues) => string
  readonly setPreference: (preference: DesktopLocalePreference) => Promise<void>
}

const fallbackContext: I18nContextValue = {
  locale: 'en', preference: 'system', ready: true,
  t: (key, values) => translate('en', key, values),
  setPreference: async () => undefined,
}
const I18nContext = createContext<I18nContextValue>(fallbackContext)

export function I18nProvider({
  children,
  api = window.loopalDesktop,
  systemLocales,
}: {
  readonly children: ReactNode
  readonly api?: Pick<LoopalDesktopAPI, 'getDesktopPreferences' | 'updateDesktopPreferences'>
  readonly systemLocales?: readonly string[]
}) {
  const [preferences, setPreferences] = useState<DesktopPreferences>(DEFAULT_DESKTOP_PREFERENCES)
  const [ready, setReady] = useState(false)
  const [languageRevision, setLanguageRevision] = useState(0)

  useEffect(() => {
    let active = true
    void api.getDesktopPreferences().then((value) => {
      if (active) setPreferences(value)
    }).catch(() => undefined).finally(() => {
      if (active) setReady(true)
    })
    return () => { active = false }
  }, [api])

  useEffect(() => {
    if (systemLocales || typeof window === 'undefined') return undefined
    const update = (): void => setLanguageRevision((value) => value + 1)
    window.addEventListener('languagechange', update)
    return () => window.removeEventListener('languagechange', update)
  }, [systemLocales])

  const locale = resolveLocale(
    preferences.locale,
    systemLocales ?? (typeof navigator === 'undefined' ? [] : navigator.languages),
  )
  void languageRevision

  useEffect(() => {
    document.documentElement.lang = locale
  }, [locale])

  const setPreference = useCallback(async (preference: DesktopLocalePreference) => {
    const persisted = await api.updateDesktopPreferences({ locale: preference })
    setPreferences(persisted)
  }, [api])
  const t = useCallback(
    (key: MessageKey, values?: TranslationValues) => translate(locale, key, values),
    [locale],
  )
  const value = useMemo<I18nContextValue>(() => ({
    locale, preference: preferences.locale, ready, setPreference, t,
  }), [locale, preferences.locale, ready, setPreference, t])

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>
}

export function useI18n(): I18nContextValue {
  return useContext(I18nContext)
}
