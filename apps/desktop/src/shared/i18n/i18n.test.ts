import { resolveLocale, translate, type MessageKey } from './'

describe('desktop internationalization', () => {
  it('resolves explicit and system locales with an English fallback', () => {
    expect(resolveLocale('zh-CN', ['en-US'])).toBe('zh-CN')
    expect(resolveLocale('en', ['zh-CN'])).toBe('en')
    expect(resolveLocale('system', ['zh-Hans-CN', 'en-US'])).toBe('zh-CN')
    expect(resolveLocale('system', ['ZH_tw'])).toBe('zh-CN')
    expect(resolveLocale('system', ['fr-FR'])).toBe('en')
    expect(resolveLocale('system', [])).toBe('en')
  })

  it('provides complete typed English and Chinese catalogs', () => {
    const keys: MessageKey[] = [
      'activity.conversation', 'activity.federation', 'activity.settings',
      'language.system', 'language.en', 'language.zh-CN', 'settings.language',
      'settings.languageHint', 'common.close', 'common.save', 'common.cancel',
      'common.loading', 'common.error',
      'settings.skills.navigation', 'settings.skills.editor.save',
      'settings.plugins.restart',
    ]
    for (const key of keys) {
      expect(translate('en', key)).toBeTruthy()
      expect(translate('zh-CN', key)).toBeTruthy()
    }
    expect(translate('zh-CN', 'settings.language')).toBe('显示语言')
    expect(translate('zh-CN', 'settings.skills.navigation')).toBe('Skills 与 Plugins')
  })
})
