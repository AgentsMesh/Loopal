import { useEffect } from 'react'
import { type WorkbenchArea } from './workbench-view-state'

export function useWorkbenchShortcuts(actions: {
  readonly selectArea: (area: WorkbenchArea) => void
  readonly toggleSidebar: () => void
  readonly toggleSettings: () => void
}): void {
  useEffect(() => {
    const listener = (event: KeyboardEvent): void => {
      if (event.isComposing) return
      if (!event.metaKey && !event.ctrlKey) return
      const key = event.key.toLowerCase()
      const action = key === '1' ? () => actions.selectArea('conversation')
        : key === '2' ? () => actions.selectArea('federation')
          : key === 'b' ? actions.toggleSidebar
            : key === ',' ? actions.toggleSettings : undefined
      if (!action) return
      event.preventDefault()
      action()
    }
    window.addEventListener('keydown', listener)
    return () => window.removeEventListener('keydown', listener)
  }, [actions])
}
