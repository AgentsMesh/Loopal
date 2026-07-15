import {
  DEFAULT_DESKTOP_PREFERENCES,
  DesktopLocalePreferenceSchema,
  DesktopPreferencesSchema,
} from './desktop-preferences-contracts'

describe('desktop preference contracts', () => {
  it('accepts only the supported application locale preferences', () => {
    for (const locale of ['system', 'en', 'zh-CN']) {
      expect(DesktopPreferencesSchema.parse({ locale })).toEqual({ locale })
    }
    expect(DesktopLocalePreferenceSchema.safeParse('zh-TW').success).toBe(false)
    expect(DesktopPreferencesSchema.safeParse({ locale: 'en', extra: true }).success).toBe(false)
    expect(DEFAULT_DESKTOP_PREFERENCES).toEqual({ locale: 'system' })
  })
})
