import { useState, type KeyboardEvent, type MouseEvent, type RefObject } from 'react'
import { type SessionStatus, type SessionSummary } from '../../../../shared/contracts'
import { type MessageKey } from '../../../../shared/i18n'
import { isSessionLive } from '../../../../shared/contracts/session-lifecycle'
import { useI18n } from '../../../browser/i18n-context'
import {
  SessionContextMenu, type SessionContextMenuTarget, type SessionFederationActions,
} from './session-context-menu'

const statusLabels: Record<SessionStatus, MessageKey> = {
  stopped: 'session.status.stopped',
  starting: 'session.status.starting',
  running: 'session.status.running',
  waiting: 'session.status.waiting',
  failed: 'session.status.failed',
  archived: 'session.status.archived',
}

interface SessionNavigatorProps {
  readonly currentSessions: readonly SessionSummary[]
  readonly searchResults: readonly SessionSummary[]
  readonly activeSessionId?: string
  readonly query: string
  readonly searchRef: RefObject<HTMLInputElement | null>
  readonly canCreate: boolean
  readonly federation: SessionFederationActions
  readonly onQueryChange: (query: string) => void
  readonly onOpenSession: (sessionId: string) => Promise<unknown>
  readonly onRequestCreate: () => void
}

export function SessionNavigator(props: SessionNavigatorProps): React.JSX.Element {
  const { t } = useI18n()
  const [menu, setMenu] = useState<SessionContextMenuTarget>()
  const searching = Boolean(props.query.trim())
  const current = searching ? props.searchResults.filter(isSessionLive) : props.currentSessions
  const history = searching ? props.searchResults.filter((session) => !isSessionLive(session)) : []
  const openPointerMenu = (event: MouseEvent<HTMLButtonElement>, session: SessionSummary): void => {
    event.preventDefault()
    event.stopPropagation()
    setMenu({ session, trigger: event.currentTarget, x: event.clientX, y: event.clientY })
  }
  const openKeyboardMenu = (
    event: KeyboardEvent<HTMLButtonElement>, session: SessionSummary,
  ): void => {
    if (event.key !== 'ContextMenu' && !(event.shiftKey && event.key === 'F10')) return
    event.preventDefault()
    event.stopPropagation()
    const bounds = event.currentTarget.getBoundingClientRect()
    setMenu({ session, trigger: event.currentTarget, x: bounds.left + 16, y: bounds.top + 24 })
  }
  const renderCard = (session: SessionSummary): React.JSX.Element => (
    <button
      key={session.id}
      className={`session-card ${session.id === props.activeSessionId ? 'selected' : ''}`}
      onClick={() => void props.onOpenSession(session.id)}
      onContextMenu={(event) => openPointerMenu(event, session)}
      onKeyDown={(event) => openKeyboardMenu(event, session)}
      aria-haspopup="menu"
      aria-expanded={menu?.session.id === session.id}
      aria-controls={menu?.session.id === session.id ? 'session-context-menu' : undefined}
      data-session-id={session.id}
    >
      <span className={`status-dot status-${session.status}`} />
      <span className="session-copy">
        <strong>{session.title}</strong>
        <small>{t(statusLabels[session.status])}</small>
      </span>
      {session.attention && session.attention !== 'completed'
        && <span className="attention" title={session.attention}>!</span>}
    </button>
  )
  const renderSection = (
    testId: 'current-session-list' | 'history-session-list', label: string,
    sessions: readonly SessionSummary[],
  ): React.JSX.Element | undefined => sessions.length ? (
    <section className={`session-section ${testId === 'history-session-list'
      ? 'session-section-history' : ''}`} data-testid={testId}
      aria-labelledby={`${testId}-heading`}>
      <h2 id={`${testId}-heading`}><span>{label}</span><small>{sessions.length}</small></h2>
      {sessions.map(renderCard)}
    </section>
  ) : undefined
  return (
    <aside className="session-navigator">
      <header className="navigator-header">
        <div>
          <span className="eyebrow">{t('navigator.workbench')}</span>
          <h1>{t('navigator.sessions')}</h1>
        </div>
        <button
          className="new-session"
          aria-label={t('navigator.newSession')}
          disabled={!props.canCreate}
          onClick={props.onRequestCreate}
        >＋</button>
      </header>
      <label className="session-search">
        <span>⌕</span>
        <input
          ref={props.searchRef}
          aria-label={t('navigator.search')}
          placeholder={t('navigator.searchPlaceholder')}
          value={props.query}
          onChange={(event) => props.onQueryChange(event.target.value)}
        />
      </label>
      <div className="session-list" data-testid="session-list">
        {renderSection('current-session-list', t('navigator.currentSessions'), current)}
        {renderSection('history-session-list', t('navigator.historySessions'), history)}
        {!searching && current.length === 0 && (
          <p className="session-empty-state">{t('navigator.noCurrentSessions')}</p>
        )}
        {searching && current.length + history.length === 0 && (
          <p className="session-empty-state">
            {t('navigator.noSearchResults', { query: props.query.trim() })}
          </p>
        )}
      </div>
      {menu && <SessionContextMenu target={menu} federation={props.federation}
        onClose={() => setMenu(undefined)} />}
    </aside>
  )
}
