import { useEffect, useMemo, useState } from 'react'
import { useI18n } from '../../../browser/i18n-context'

export type SettingsSectionId =
  | 'appearance'
  | 'loopal'
  | 'providers'
  | 'skills'
  | 'mcp'
  | 'agent'
  | 'runtime'
  | 'federation'

interface NavigationItem {
  readonly id: SettingsSectionId
  readonly label: string
  readonly searchTerms: string
}

interface NavigationGroup {
  readonly label: string
  readonly searchTerms: string
  readonly items: readonly NavigationItem[]
}

export function SettingsNavigation(props: {
  readonly active: SettingsSectionId
  readonly onSelect: (section: SettingsSectionId) => void
  readonly onVisibilityChange: (hasResults: boolean) => void
}): React.JSX.Element {
  const { locale, t } = useI18n()
  const [query, setQuery] = useState('')
  const groups = useMemo<readonly NavigationGroup[]>(() => [{
    label: t('settings.category.desktop'),
    searchTerms: 'desktop user 桌面 用户',
    items: [{ id: 'appearance', label: t('settings.appearance'), searchTerms: 'appearance 外观' }],
  }, {
    label: t('settings.category.loopal'),
    searchTerms: 'loopal user global 用户 全局',
    items: [
      { id: 'loopal', label: t('settings.loopal.defaults.navigation'), searchTerms: 'defaults 默认' },
      { id: 'providers', label: t('settings.loopal.providers.navigation'), searchTerms: 'model provider 模型 提供商' },
      { id: 'skills', label: t('settings.skills.navigation'), searchTerms: 'skill skills plugin plugins 技能 插件 全局 来源' },
      { id: 'mcp', label: t('settings.mcp.navigation'), searchTerms: 'mcp server workspace 服务器 工作区' },
    ],
  }, {
    label: t('settings.category.session'),
    searchTerms: 'session current 会话 当前',
    items: [
      { id: 'agent', label: t('settings.currentAgent'), searchTerms: 'agent live 智能体 实时' },
      { id: 'runtime', label: t('settings.runtimeMcp'), searchTerms: 'runtime diagnostics 运行时 诊断' },
    ],
  }, {
    label: t('settings.category.federation'),
    searchTerms: 'federation application 联邦 应用',
    items: [{ id: 'federation', label: t('settings.metahub.navigation'), searchTerms: 'metahub hub 联邦' }],
  }], [t])
  const filtered = filterGroups(groups, query, locale)
  const visibleItems = filtered.flatMap((group) => group.items)
  useEffect(() => {
    props.onVisibilityChange(visibleItems.length > 0)
    const first = visibleItems[0]
    if (first && !visibleItems.some((item) => item.id === props.active)) {
      props.onSelect(first.id)
    }
  }, [props.active, props.onSelect, props.onVisibilityChange,
    visibleItems.map((item) => item.id).join(':')])
  return <aside className="settings-navigation" data-testid="settings-navigation">
    <label className="settings-search">
      <span className="sr-only">{t('settings.search')}</span>
      <input type="search" value={query} data-testid="settings-search"
        aria-label={t('settings.search')} placeholder={t('settings.search')}
        onChange={(event) => setQuery(event.target.value)} />
    </label>
    <nav aria-label={t('settings.sections')}>
      {filtered.map((group) => <section className="settings-nav-group" key={group.label}>
        <h3>{group.label}</h3>
        <div role="tablist" aria-orientation="vertical">
          {group.items.map((item) => <button key={item.id} id={`settings-nav-${item.id}`}
            type="button" role="tab" data-section={item.id}
            aria-selected={props.active === item.id}
            aria-controls={`settings-section-${item.id}`}
            tabIndex={props.active === item.id ? 0 : -1}
            onClick={() => props.onSelect(item.id)}
            onKeyDown={(event) => navigateByKeyboard(event, props.onSelect)}>
            {item.label}
          </button>)}
        </div>
      </section>)}
      {filtered.length === 0 && <p className="settings-nav-empty">
        {t('settings.noResults')}
      </p>}
    </nav>
  </aside>
}

function filterGroups(
  groups: readonly NavigationGroup[], query: string, locale: string,
): readonly NavigationGroup[] {
  const needle = query.trim().toLocaleLowerCase(locale)
  if (!needle) return groups
  return groups.flatMap((group) => {
    const groupMatches = `${group.label} ${group.searchTerms}`
      .toLocaleLowerCase(locale).includes(needle)
    const items = groupMatches ? group.items : group.items.filter((item) => (
      `${item.label} ${item.searchTerms}`.toLocaleLowerCase(locale).includes(needle)
    ))
    return items.length ? [{ ...group, items }] : []
  })
}

function navigateByKeyboard(
  event: React.KeyboardEvent<HTMLButtonElement>,
  onSelect: (section: SettingsSectionId) => void,
): void {
  if (!['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) return
  const root = event.currentTarget.closest('[data-testid="settings-navigation"]')
  const tabs = [...(root?.querySelectorAll<HTMLButtonElement>('[role="tab"]') ?? [])]
  if (!tabs.length) return
  const current = tabs.indexOf(event.currentTarget)
  const next = event.key === 'Home' ? 0 : event.key === 'End' ? tabs.length - 1
    : (current + (event.key === 'ArrowDown' ? 1 : -1) + tabs.length) % tabs.length
  event.preventDefault()
  const target = tabs[next]
  if (!target) return
  target.focus()
  onSelect(target.dataset.section as SettingsSectionId)
}
