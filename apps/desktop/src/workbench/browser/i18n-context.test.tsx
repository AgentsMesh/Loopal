import { act, renderHook, waitFor } from '@testing-library/react'
import { type ReactNode } from 'react'
import { I18nProvider, useI18n } from './i18n-context'

describe('I18nProvider', () => {
  it('loads, renders, persists, and applies the selected locale', async () => {
    let locale = 'zh-CN' as const
    const api = {
      getDesktopPreferences: vi.fn(async () => ({ locale })),
      updateDesktopPreferences: vi.fn(async (input: { locale: 'system' | 'en' | 'zh-CN' }) => {
        locale = input.locale as 'zh-CN'
        return { locale: input.locale }
      }),
    }
    const wrapper = ({ children }: { children: ReactNode }) => (
      <I18nProvider api={api} systemLocales={['en-US']}>{children}</I18nProvider>
    )
    const result = renderHook(useI18n, { wrapper }).result
    await waitFor(() => expect(result.current.ready).toBe(true))
    expect(result.current.locale).toBe('zh-CN')
    expect(result.current.t('activity.sessions')).toBe('会话')
    expect(document.documentElement.lang).toBe('zh-CN')

    await act(() => result.current.setPreference('en'))
    expect(api.updateDesktopPreferences).toHaveBeenCalledWith({ locale: 'en' })
    expect(result.current.t('activity.sessions')).toBe('Sessions')
    expect(document.documentElement.lang).toBe('en')
  })

  it('uses system language changes and survives preference read failure', async () => {
    const api = {
      getDesktopPreferences: vi.fn(async () => { throw new Error('unavailable') }),
      updateDesktopPreferences: vi.fn(async () => ({ locale: 'system' as const })),
    }
    const wrapper = ({ children }: { children: ReactNode }) => (
      <I18nProvider api={api} systemLocales={['zh-Hans']}>{children}</I18nProvider>
    )
    const result = renderHook(useI18n, { wrapper }).result
    await waitFor(() => expect(result.current.ready).toBe(true))
    expect(result.current.preference).toBe('system')
    expect(result.current.locale).toBe('zh-CN')
  })

  it('provides an English fallback to independently rendered components', () => {
    const result = renderHook(useI18n).result
    expect(result.current.locale).toBe('en')
    expect(result.current.t('activity.sessions')).toBe('Sessions')
  })
})
