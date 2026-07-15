import { type DesktopLocalePreference } from '../contracts/desktop-preferences-contracts'
import { EN_MESSAGES, type MessageKey } from './i18n-catalog-en'
import { ZH_CN_MESSAGES } from './i18n-catalog-zh-cn'

export { type MessageKey } from './i18n-catalog-en'

export const SUPPORTED_LOCALES = ['en', 'zh-CN'] as const
export type SupportedLocale = typeof SUPPORTED_LOCALES[number]

type Catalog = Readonly<Record<MessageKey, string>>
const catalogs: Readonly<Record<SupportedLocale, Catalog>> = {
  en: EN_MESSAGES,
  'zh-CN': ZH_CN_MESSAGES,
}

export function resolveLocale(
  preference: DesktopLocalePreference,
  systemLocales: readonly string[] | string = systemLanguages(),
): SupportedLocale {
  if (preference !== 'system') return preference
  const values = typeof systemLocales === 'string' ? [systemLocales] : systemLocales
  return values.some((value) => normalizeTag(value).startsWith('zh')) ? 'zh-CN' : 'en'
}

export function translate(
  locale: SupportedLocale,
  key: MessageKey,
  values: Readonly<Record<string, string | number>> = {},
): string {
  const message = catalogs[locale][key] ?? catalogs.en[key] ?? key
  return message.replace(/\{([^}]+)\}/g, (match, name: string) => (
    Object.hasOwn(values, name) ? String(values[name]) : match
  ))
}

function systemLanguages(): readonly string[] {
  return typeof navigator === 'undefined' ? [] : navigator.languages
}

function normalizeTag(value: string): string {
  return value.trim().replaceAll('_', '-').toLowerCase()
}
