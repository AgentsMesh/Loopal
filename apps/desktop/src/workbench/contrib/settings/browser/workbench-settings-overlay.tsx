import { useEffect, useRef } from 'react'
import { type LoopalDesktopAPI } from '../../../../shared/contracts'
import { type DesktopPreferences } from './desktop-preferences'
import { useI18n } from '../../../browser/i18n-context'
import { SessionSettingsView } from './settings-view'
import { type useAgentControl } from '../../agents/browser/use-agent-control'
import { type useWorkbenchController } from '../../../browser/use-workbench-controller'

export function WorkbenchSettingsOverlay(props: {
  readonly api: LoopalDesktopAPI
  readonly controller: ReturnType<typeof useWorkbenchController>
  readonly agentControl: ReturnType<typeof useAgentControl>
  readonly activeAgentId: string
  readonly canControlAgent: boolean
  readonly preferences: DesktopPreferences
  readonly onPreferences: (patch: Partial<DesktopPreferences>) => void
  readonly onSelectAgent: (agentId: string) => void
  readonly onClose: () => void
}): React.JSX.Element {
  const { t } = useI18n()
  const overlayRef = useRef<HTMLDivElement>(null)
  const closeRef = useRef(props.onClose)
  closeRef.current = props.onClose
  useEffect(() => {
    const overlay = overlayRef.current
    const previous = document.activeElement as HTMLElement | null
    if (!overlay) return
    const onKeyDown = (event: KeyboardEvent): void => {
      if (event.key === 'Escape') {
        event.preventDefault()
        closeRef.current()
        return
      }
      if (event.key !== 'Tab') return
      const focusable = focusableElements(overlay)
      if (!focusable.length) return
      const index = focusable.indexOf(document.activeElement as HTMLElement)
      const boundary = event.shiftKey ? index <= 0 : index === focusable.length - 1
      if (!boundary && index >= 0) return
      event.preventDefault()
      focusable[event.shiftKey ? focusable.length - 1 : 0]?.focus()
    }
    overlay.addEventListener('keydown', onKeyDown)
    requestAnimationFrame(() => focusableElements(overlay)[0]?.focus())
    return () => {
      overlay.removeEventListener('keydown', onKeyDown)
      requestAnimationFrame(() => { if (previous?.isConnected) previous.focus() })
    }
  }, [])
  const projection = props.controller.projection
  return <div ref={overlayRef} className="settings-overlay" data-workspace="settings"
    role="dialog" aria-modal="true" aria-label={t('settings.title')}>
    <SessionSettingsView api={props.api} runtimes={projection.runtimes}
      {...(projection.detail !== undefined ? { detail: projection.detail } : {})}
      hostStatus={projection.hostStatus} selectedAgentId={props.activeAgentId}
      onSelectAgent={props.onSelectAgent} canControl={props.canControlAgent}
      busy={props.agentControl.busy} preferences={props.preferences}
      onPreferences={props.onPreferences}
      onInterrupt={() => void props.agentControl.interrupt(props.activeAgentId)}
      onControl={(command) => void props.agentControl.control(props.activeAgentId, command)}
      onClose={props.onClose} />
  </div>
}

function focusableElements(root: HTMLElement): HTMLElement[] {
  return [...root.querySelectorAll<HTMLElement>(
    'button:not(:disabled), input:not(:disabled), select:not(:disabled), '
      + 'textarea:not(:disabled), a[href], [tabindex]:not([tabindex="-1"])',
  )].filter((element) => !element.hidden && element.getClientRects().length > 0)
}
