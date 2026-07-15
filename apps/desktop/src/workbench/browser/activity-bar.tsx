import { useI18n } from './i18n-context'
import { WorkbenchIcon, type WorkbenchIconName } from './workbench-icon'
import { type WorkbenchArea } from './workbench-view-state'

interface ActivityBarProps {
  readonly activeArea: WorkbenchArea
  readonly sidebarVisible: boolean
  readonly attentionCount: number
  readonly onActivate: (area: WorkbenchArea) => void
  readonly onToggleSidebar: () => void
  readonly onOpenAttention: () => void
  readonly settingsOpen: boolean
  readonly onOpenSettings: () => void
}

const productAreas = [
  ['conversation', 'activity.conversation', 'conversation'],
  ['federation', 'activity.federation', 'federation'],
] as const

export function ActivityBar(props: ActivityBarProps): React.JSX.Element {
  const { t } = useI18n()
  return (
    <nav className="activity-bar" aria-label={t('activity.primary')}>
      <div className="brand-mark" aria-label="Loopal">L</div>
      <ActivityGroup label={t('activity.productAreas')} items={productAreas} {...props} />
      <span className="activity-spacer" />
      {props.attentionCount > 0 && (
        <button
          className="activity-badge"
          aria-label={t('activity.pendingRequests', { count: props.attentionCount })}
          onClick={props.onOpenAttention}
        >
          {props.attentionCount}
        </button>
      )}
      <button
        className={`activity ${props.sidebarVisible ? 'active' : ''}`}
        aria-label={t('activity.toggleSidebar')}
        aria-pressed={props.sidebarVisible}
        onClick={props.onToggleSidebar}
      ><WorkbenchIcon name="sidebar" /></button>
      <button
        className={`activity ${props.settingsOpen ? 'active' : ''}`}
        aria-label={t('activity.settings')}
        aria-pressed={props.settingsOpen}
        onClick={props.onOpenSettings}
      ><WorkbenchIcon name="settings" /></button>
    </nav>
  )
}

function ActivityGroup(props: ActivityBarProps & {
  readonly label: string
  readonly items: readonly (readonly [
    WorkbenchArea, Parameters<ReturnType<typeof useI18n>['t']>[0], WorkbenchIconName,
  ])[]
}): React.JSX.Element {
  const { t } = useI18n()
  return <div className="activity-group" role="group" aria-label={props.label}>
    {props.items.map(([id, key, icon]) => (
      <button key={id} className={`activity ${props.activeArea === id ? 'active' : ''}`}
        aria-label={t(key)} aria-pressed={props.activeArea === id}
        onClick={() => props.onActivate(id)}>
        <WorkbenchIcon name={icon} />
      </button>
    ))}
  </div>
}
