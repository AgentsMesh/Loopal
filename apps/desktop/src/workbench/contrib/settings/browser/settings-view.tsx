import {
  type AgentControlCommand, type HostStatus, type LoopalDesktopAPI,
  type RuntimeSummary, type SessionDetail,
} from '../../../../shared/contracts'
import { useState } from 'react'
import { canRestartSession } from '../../../../shared/contracts/session-lifecycle'
import { AgentControlPanel } from '../../agents/browser/agent-control-panel'
import {
  type DesktopPreferences,
} from './desktop-preferences'
import { DiagnosticsInspector } from '../../session-panels/browser/diagnostics-inspector'
import { DesktopAppearanceSettings } from './desktop-appearance-settings'
import { useI18n } from '../../../browser/i18n-context'
import { LoopalDefaultSettings } from './loopal-default-settings'
import { LoopalMcpSettings } from './loopal-mcp-settings'
import { LoopalSkillPluginSettings } from './loopal-skill-plugin-settings'
import { MetaHubSettingsSection } from './metahub-settings-section'
import { resolveMetaHubRuntimeTarget } from '../../sessions/browser/session-runtime-target'
import { SettingsNavigation, type SettingsSectionId } from './settings-navigation'
import './settings-view.css'

export interface SettingsViewProps {
  readonly detail?: SessionDetail
  readonly hostStatus: HostStatus
  readonly selectedAgentId: string
  readonly onSelectAgent: (agentId: string) => void
  readonly canControl: boolean
  readonly busy: boolean
  readonly preferences: DesktopPreferences
  readonly onPreferences: (patch: Partial<DesktopPreferences>) => void
  readonly onInterrupt: () => void
  readonly onControl: (command: AgentControlCommand) => void
  readonly onClose: () => void
  readonly metaHubSettings?: React.ReactNode
  readonly loopalDefaults?: React.ReactNode
  readonly loopalProviders?: React.ReactNode
  readonly mcpSettings?: React.ReactNode
  readonly renderLoopalSettings?: (section: 'defaults' | 'providers' | 'hidden') => React.ReactNode
  readonly renderMcpSettings?: (visible: boolean) => React.ReactNode
  readonly renderSkillPluginSettings?: (visible: boolean) => React.ReactNode
  readonly renderMetaHubSettings?: (visible: boolean) => React.ReactNode
}

export function SessionSettingsView(props: SettingsViewProps & {
  readonly api: LoopalDesktopAPI
  readonly runtimes: readonly RuntimeSummary[]
}): React.JSX.Element {
  const target = props.detail ? resolveMetaHubRuntimeTarget(
    props.detail.session.id, [props.detail.session], props.runtimes,
  ) : undefined
  return <SettingsView {...props} renderMcpSettings={(visible) => (
    <LoopalMcpSettings api={props.api} visible={visible}
      {...(props.detail ? { workspaceId: props.detail.session.workspaceId } : {})} />
  )} renderLoopalSettings={(section) => (
    <LoopalDefaultSettings api={props.api} section={section}
      {...(props.detail ? {
        workspaceId: props.detail.session.workspaceId,
        ...(canRestartSession(props.detail.session) ? { sessionId: props.detail.session.id } : {}),
      } : {})} />
  )} renderMetaHubSettings={(visible) => (
    <MetaHubSettingsSection
      visible={visible}
      api={props.api} {...(target ? { target } : {})}
      {...(props.detail?.metaHub ? { initialState: props.detail.metaHub } : {})}
    />
  )} renderSkillPluginSettings={(visible) => (
    <LoopalSkillPluginSettings api={props.api} visible={visible}
      {...(props.detail ? { workspaceId: props.detail.session.workspaceId } : {})} />
  )} />
}

export function SettingsView(props: SettingsViewProps): React.JSX.Element {
  const { t } = useI18n()
  const [activeSection, setActiveSection] = useState<SettingsSectionId>('appearance')
  const [hasSearchResults, setHasSearchResults] = useState(true)
  const selected = props.detail?.agents.find((agent) => agent.id === props.selectedAgentId)
    ?? props.detail?.agents.find((agent) => !agent.parentId)
    ?? props.detail?.agents[0]
  return (
    <section className="settings-view" data-testid="settings-pane" aria-label={t('settings.title')}>
      <header className="settings-header">
        <div><span className="eyebrow">Loopal Desktop</span><h2>{t('settings.title')}</h2></div>
        <button aria-label={t('settings.close')} onClick={props.onClose}>×</button>
      </header>
      <div className="settings-body">
        <SettingsNavigation active={activeSection} onSelect={setActiveSection}
          onVisibilityChange={setHasSearchResults} />
        <main className="settings-scroll" role="tabpanel" tabIndex={0}
          id={`settings-section-${activeSection}`}
          {...(hasSearchResults
            ? { 'aria-labelledby': `settings-nav-${activeSection}` }
            : { 'aria-label': t('settings.noResults') })}
          data-testid="settings-section-content">
          {hasSearchResults && <SettingsScope section={activeSection} />}
          {props.renderLoopalSettings?.(hasSearchResults
            ? activeSection === 'loopal' ? 'defaults'
              : activeSection === 'providers' ? 'providers' : 'hidden'
            : 'hidden')}
          {props.renderMcpSettings?.(hasSearchResults && activeSection === 'mcp')}
          {props.renderSkillPluginSettings?.(hasSearchResults && activeSection === 'skills')}
          {props.renderMetaHubSettings?.(hasSearchResults && activeSection === 'federation')}
          {hasSearchResults && !usesStatefulRenderer(props, activeSection)
            && <SettingsContent {...props} active={activeSection}
              {...(selected ? { selected } : {})} />}
          {!hasSearchResults && <p className="settings-content-empty" role="status">
            {t('settings.noResults')}
          </p>}
        </main>
      </div>
    </section>
  )
}

function usesStatefulRenderer(props: SettingsViewProps, section: SettingsSectionId): boolean {
  if ((section === 'loopal' || section === 'providers') && props.renderLoopalSettings) return true
  if (section === 'mcp' && props.renderMcpSettings) return true
  if (section === 'skills' && props.renderSkillPluginSettings) return true
  return section === 'federation' && props.renderMetaHubSettings !== undefined
}

function SettingsScope(props: { readonly section: SettingsSectionId }): React.JSX.Element {
  const { t } = useI18n()
  const key = props.section === 'appearance' ? 'settings.scope.desktop'
    : props.section === 'loopal' || props.section === 'providers' ? 'settings.scope.loopal'
      : props.section === 'skills' ? 'settings.skills.scope'
      : props.section === 'mcp' ? 'settings.scope.mcp'
        : props.section === 'federation' ? 'settings.scope.metahub'
          : 'settings.scope.session'
  return <p className="settings-scope">{t(key)}</p>
}

function SettingsContent(props: SettingsViewProps & {
  readonly active: SettingsSectionId
  readonly selected?: NonNullable<SettingsViewProps['detail']>['agents'][number]
}): React.JSX.Element | null {
  const { t } = useI18n()
  if (props.active === 'appearance') return <SettingsSection title={t('settings.appearance')}>
    <DesktopAppearanceSettings preferences={props.preferences} onPreferences={props.onPreferences} />
  </SettingsSection>
  if (props.active === 'loopal') return <>{props.loopalDefaults}</>
  if (props.active === 'providers') return <>{props.loopalProviders}</>
  if (props.active === 'mcp') return <>{props.mcpSettings}</>
  if (props.active === 'skills') return null
  if (props.active === 'runtime') return <SettingsSection title={t('settings.runtimeMcp')}>
    <DiagnosticsInspector hostStatus={props.hostStatus} detail={props.detail}
      agentId={props.selected?.id} canControl={props.canControl} busy={props.busy}
      onControl={props.onControl} />
  </SettingsSection>
  if (props.active === 'federation') return <>{props.metaHubSettings}</>
  return <SettingsSection title={t('settings.currentAgent')}>
    {props.detail && <dl className="settings-session-metadata">
      <div><dt>{t('settings.session')}</dt><dd>{props.detail.session.id}</dd></div>
      <div><dt>{t('settings.status')}</dt><dd>{props.detail.session.status}</dd></div>
      <div><dt>{t('settings.host')}</dt><dd>{props.hostStatus}</dd></div>
    </dl>}
    {props.detail && props.detail.agents.length > 0 && <label className="settings-field">
      <span>{t('settings.configureAgent')}</span>
      <select aria-label={t('settings.settingsAgent')} value={props.selected?.id}
        onChange={(event) => props.onSelectAgent(event.target.value)}>
        {props.detail.agents.map((agent) => <option key={agent.id} value={agent.id}>
          {agent.name} · {agent.status}
        </option>)}
      </select>
    </label>}
    {props.selected ? <AgentControlPanel agent={props.selected}
      disabled={!props.canControl} busy={props.busy} onInterrupt={props.onInterrupt}
      onControl={props.onControl} />
      : <p className="muted">{t('settings.selectLive')}</p>}
  </SettingsSection>
}

function SettingsSection(props: {
  readonly title: string
  readonly children: React.ReactNode
}): React.JSX.Element {
  return <section className="settings-section"><h3>{props.title}</h3>{props.children}</section>
}
