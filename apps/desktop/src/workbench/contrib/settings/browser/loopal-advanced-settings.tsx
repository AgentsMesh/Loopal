import { useMemo, useState } from 'react'
import { type LoopalDefaultSettings } from '../../../../shared/contracts'
import { useI18n } from '../../../browser/i18n-context'

interface Props {
  readonly record: LoopalDefaultSettings
}

export function LoopalAdvancedSettings({ record }: Props): React.JSX.Element {
  const { t } = useI18n()
  const [filter, setFilter] = useState('')
  const entries = useMemo(() => {
    const query = filter.trim().toLocaleLowerCase()
    if (!query) return record.resolvedEntries
    return record.resolvedEntries.filter((entry) =>
      entry.key.toLocaleLowerCase().includes(query)
        || entry.value.toLocaleLowerCase().includes(query))
  }, [filter, record.resolvedEntries])
  return <details className="advanced-settings" data-testid="advanced-resolved-settings">
    <summary>{t('settings.loopal.advanced.title')}</summary>
    <p className="muted">{t('settings.loopal.advanced.help')}</p>
    <p className="settings-sources">{t('settings.loopal.advanced.sources', {
      sources: record.settingSources.join(' · ') || t('settings.loopal.advanced.defaults'),
    })}</p>
    <label className="settings-field"><span>{t('settings.loopal.advanced.search')}</span>
      <input type="search" aria-label={t('settings.loopal.advanced.search')} value={filter}
      onChange={(event) => setFilter(event.currentTarget.value)} /></label>
    <div className="resolved-settings-table" role="table"
      aria-label={t('settings.loopal.advanced.table')}>
      {entries.map((entry, index) => <div role="row" key={`${entry.key}:${index}`}>
        <code role="cell">{entry.key}</code><span role="cell">{entry.value}</span>
      </div>)}
      {entries.length === 0 && <p className="muted">{t('settings.loopal.advanced.empty')}</p>}
    </div>
  </details>
}
