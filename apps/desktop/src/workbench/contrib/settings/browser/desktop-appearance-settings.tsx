import { type DesktopLocalePreference } from '../../../../shared/contracts'
import { type DesktopPreferences } from './desktop-preferences'
import { useI18n } from '../../../browser/i18n-context'

export function DesktopAppearanceSettings(props: {
  readonly preferences: DesktopPreferences
  readonly onPreferences: (patch: Partial<DesktopPreferences>) => void
}): React.JSX.Element {
  const { preference, setPreference, ready, t } = useI18n()
  return (
    <>
      <label className="settings-field">
        <span>{t('settings.language')}</span>
        <select
          data-testid="desktop-language"
          aria-label={t('settings.language')}
          value={preference}
          disabled={!ready}
          onChange={(event) => void setPreference(
            event.target.value as DesktopLocalePreference,
          )}
        >
          <option value="system">{t('language.system')}</option>
          <option value="en">{t('language.en')}</option>
          <option value="zh-CN">{t('language.zh-CN')}</option>
        </select>
        <small>{t('settings.languageHint')}</small>
      </label>
      <label className="settings-field">
        <span>{t('settings.panelDensity')}</span>
        <select
          aria-label={t('settings.panelDensity')} value={props.preferences.panelDensity}
          onChange={(event) => props.onPreferences({
            panelDensity: event.target.value as DesktopPreferences['panelDensity'],
          })}
        >
          <option value="comfortable">{t('settings.comfortable')}</option>
          <option value="compact">{t('settings.compact')}</option>
        </select>
      </label>
      <label className="settings-field">
        <span>{t('settings.conversationFont', {
          size: props.preferences.conversationFontSize,
        })}</span>
        <input
          aria-label={t('settings.conversationFontSize')}
          type="range" min="11" max="18" step="1"
          value={props.preferences.conversationFontSize}
          onChange={(event) => props.onPreferences({
            conversationFontSize: Number(event.target.value),
          })}
        />
      </label>
      <label className="settings-check">
        <input
          aria-label={t('settings.showTopology')} type="checkbox"
          checked={props.preferences.showAgentTopology}
          onChange={(event) => props.onPreferences({ showAgentTopology: event.target.checked })}
        />
        <span>{t('settings.showTopologyHint')}</span>
      </label>
    </>
  )
}
