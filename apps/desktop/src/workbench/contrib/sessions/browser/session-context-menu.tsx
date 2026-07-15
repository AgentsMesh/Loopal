import { useEffect, useLayoutEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { type SessionSummary } from '../../../../shared/contracts'
import { type FederationMembership } from '../../federation/browser/federation-model'
import { useI18n } from '../../../browser/i18n-context'
export type { FederationMembership } from '../../federation/browser/federation-model'

export interface SessionFederationActions {
  readonly memberships: Readonly<Record<string, FederationMembership>>
  readonly busy: string | undefined
  readonly onJoin: (sessionId: string) => Promise<void>
  readonly onLeave: (sessionId: string) => Promise<void>
}

export interface SessionContextMenuTarget {
  readonly session: SessionSummary
  readonly trigger: HTMLButtonElement
  readonly x: number
  readonly y: number
}

export function SessionContextMenu(props: {
  readonly target: SessionContextMenuTarget
  readonly federation: SessionFederationActions
  readonly onClose: () => void
}): React.JSX.Element {
  const { t } = useI18n()
  const menuRef = useRef<HTMLDivElement>(null)
  const [position, setPosition] = useState({ x: props.target.x, y: props.target.y })
  const membership = props.federation.memberships[props.target.session.id] ?? 'unavailable'
  const connected = membership === 'connected' || membership === 'error'
  const pending = props.federation.busy === `session:${props.target.session.id}`
  const disabled = membership === 'unavailable' || props.federation.busy !== undefined

  const close = (): void => {
    props.onClose()
    props.target.trigger.focus()
  }

  useLayoutEffect(() => {
    const menu = menuRef.current
    if (!menu) return
    const bounds = menu.getBoundingClientRect()
    setPosition({
      x: Math.max(8, Math.min(props.target.x, window.innerWidth - bounds.width - 8)),
      y: Math.max(8, Math.min(props.target.y, window.innerHeight - bounds.height - 8)),
    })
    const first = menu.querySelector<HTMLElement>('[role="menuitem"]:not(:disabled)')
    ;(first ?? menu).focus()
  }, [props.target.x, props.target.y])

  useEffect(() => {
    const dismiss = (event: PointerEvent): void => {
      if (!menuRef.current?.contains(event.target as Node)) close()
    }
    document.addEventListener('pointerdown', dismiss)
    return () => document.removeEventListener('pointerdown', dismiss)
  })

  const activate = (): void => {
    if (disabled) return
    close()
    const action = connected ? props.federation.onLeave : props.federation.onJoin
    void action(props.target.session.id)
  }
  const label = membership === 'unavailable'
    ? t('navigator.federationUnavailable')
    : pending
      ? t(connected ? 'navigator.leavingFederation' : 'navigator.joiningFederation')
      : t(connected ? 'navigator.leaveFederation' : 'navigator.joinFederation')

  return createPortal(
    <div ref={menuRef} id="session-context-menu" role="menu" tabIndex={-1}
      data-testid="session-context-menu"
      aria-label={t('navigator.sessionActions', { title: props.target.session.title })}
      className="session-context-menu no-drag"
      style={{ left: position.x, top: position.y }}
      onKeyDown={(event) => {
        if (event.key === 'Escape' || event.key === 'Tab') {
          event.preventDefault(); close(); return
        }
        if (['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) {
          event.preventDefault()
          menuRef.current?.querySelector<HTMLElement>('[role="menuitem"]:not(:disabled)')?.focus()
        }
      }}>
      <button type="button" role="menuitem" disabled={disabled}
        data-testid="session-federation-action" onClick={activate}>
        <span className={`session-federation-mark state-${membership}`} aria-hidden="true" />
        {label}
      </button>
    </div>,
    document.body,
  )
}
