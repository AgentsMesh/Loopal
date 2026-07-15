import { useEffect, useId, useState } from 'react'
import {
  type AgentControlCommand, type HostStatus, type SessionDetail,
} from '../../../../shared/contracts'
import { SessionPanelContent } from './session-panel-content'
import {
  buildSessionPanelState, type SessionPanelEntry, type SessionPanelId,
} from './session-panel-state'
import { type MessageKey } from '../../../../shared/i18n'
import { useI18n } from '../../../browser/i18n-context'

const panelKeys: Record<SessionPanelId, MessageKey> = {
  agents: 'panels.agents', tasks: 'panels.tasks', background: 'panels.background',
  scheduled: 'panels.scheduled', artifacts: 'panels.artifacts', mcp: 'panels.mcp',
  diagnostics: 'panels.diagnostics',
}

interface SessionPanelZoneProps {
  readonly detail?: SessionDetail
  readonly hostStatus: HostStatus
  readonly selectedAgentId: string
  readonly onSelectAgent: (agentId: string) => void
  readonly canControl: boolean
  readonly busy: boolean
  readonly onControl: (command: AgentControlCommand) => void
  readonly showTopology: boolean
}

interface DeckSelection {
  readonly panelId?: SessionPanelId | undefined
  readonly collapsed: boolean
}

export function SessionPanelZone(props: SessionPanelZoneProps): React.JSX.Element | null {
  const { t } = useI18n()
  const state = buildSessionPanelState(props)
  const sessionId = props.detail?.session.id
  const selectionKey = sessionId ?? '__empty__'
  const [selections, setSelections] = useState<Record<string, DeckSelection>>({})
  const scoped = selections[selectionKey] ?? { collapsed: true }
  const active = state.panels.find((panel) => panel.id === scoped.panelId) ?? state.panels[0]
  const baseId = useId()
  const update = (next: DeckSelection): void => setSelections((current) => ({
    ...current, [selectionKey]: next,
  }))
  useEffect(() => {
    if (active && scoped.panelId !== active.id) {
      update({ ...scoped, panelId: active.id })
    }
  }, [active?.id, scoped.panelId, sessionId])
  if (!active) return null
  const collapsed = scoped.collapsed
  const select = (panelId: SessionPanelId): void => {
    update({
      ...scoped, panelId,
      collapsed: panelId === active.id && !collapsed ? true : false,
    })
  }
  const toggle = (): void => {
    update({ ...scoped, panelId: active.id, collapsed: !collapsed })
  }
  return (
    <section
      className="session-panel-deck" data-testid="session-panel-zone"
      aria-label={t('panels.label')} onKeyDown={(event) => {
        if (event.key === 'Escape' && !scoped.collapsed) toggle()
      }}
    >
      <div className="session-panel-bar">
        <div className="session-panel-tabs" role="tablist" aria-label={t('panels.label')}>
          {state.panels.map((panel) => (
            <PanelTab
              key={panel.id} panel={panel} active={panel.id === active.id}
              expanded={panel.id === active.id && !collapsed}
              baseId={baseId} onSelect={select}
            />
          ))}
        </div>
        <button
          className="session-panel-toggle" aria-expanded={!collapsed}
          aria-controls={`${baseId}-${active.id}-panel`}
          aria-label={collapsed ? t('panels.expand') : t('panels.collapse')}
          onClick={toggle}
        >{collapsed ? '⌃' : '⌄'}</button>
      </div>
      {state.panels.map((panel) => (
        <div
          key={panel.id} className="session-panel-content" role="tabpanel"
          id={`${baseId}-${panel.id}-panel`} aria-labelledby={`${baseId}-${panel.id}-tab`}
          data-panel={panel.id} data-surface-id={panel.id}
          hidden={collapsed || panel.id !== active.id}
        >
          <SessionPanelContent panelId={panel.id}
            {...(props.detail ? { detail: props.detail } : {})}
            hostStatus={props.hostStatus} state={state} canControl={props.canControl}
            busy={props.busy} onControl={props.onControl}
            onSelectAgent={props.onSelectAgent} />
        </div>
      ))}
    </section>
  )
}

function PanelTab(props: {
  readonly panel: SessionPanelEntry
  readonly active: boolean
  readonly expanded: boolean
  readonly baseId: string
  readonly onSelect: (id: SessionPanelId) => void
}): React.JSX.Element {
  const { t } = useI18n()
  return (
    <button
      role="tab" id={`${props.baseId}-${props.panel.id}-tab`}
      aria-controls={`${props.baseId}-${props.panel.id}-panel`}
      aria-selected={props.active} aria-expanded={props.expanded}
      tabIndex={props.active ? 0 : -1}
      className={props.panel.alert ? 'has-alert' : ''}
      onClick={() => props.onSelect(props.panel.id)} onKeyDown={moveTabFocus}
    >
      <span>{t(panelKeys[props.panel.id])}</span>
      {props.panel.count > 0 && <small aria-hidden>{props.panel.count}</small>}
    </button>
  )
}

function moveTabFocus(event: React.KeyboardEvent<HTMLButtonElement>): void {
  if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return
  const tabs = Array.from(event.currentTarget.parentElement
    ?.querySelectorAll<HTMLButtonElement>('[role="tab"]') ?? [])
  const current = tabs.indexOf(event.currentTarget)
  const next = event.key === 'Home' ? 0 : event.key === 'End' ? tabs.length - 1
    : (current + (event.key === 'ArrowRight' ? 1 : -1) + tabs.length) % tabs.length
  event.preventDefault()
  if (next === current) return
  tabs[next]?.click()
  tabs[next]?.focus()
}
