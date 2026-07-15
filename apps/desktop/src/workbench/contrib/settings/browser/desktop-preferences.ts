import { useCallback, useState } from 'react'

export interface DesktopPreferences {
  readonly panelDensity: 'comfortable' | 'compact'
  readonly conversationFontSize: number
  readonly showAgentTopology: boolean
}

export const DEFAULT_DESKTOP_PREFERENCES: DesktopPreferences = {
  panelDensity: 'comfortable',
  conversationFontSize: 13,
  showAgentTopology: true,
}

const STORAGE_KEY = 'loopal.desktop.preferences.v1'

export function useDesktopPreferences(): readonly [
  DesktopPreferences,
  (patch: Partial<DesktopPreferences>) => void,
] {
  const [preferences, setPreferences] = useState(loadPreferences)
  const update = useCallback((patch: Partial<DesktopPreferences>): void => {
    setPreferences((current) => {
      const next = normalizePreferences({ ...current, ...patch })
      try { localStorage.setItem(STORAGE_KEY, JSON.stringify(next)) } catch { /* unavailable */ }
      return next
    })
  }, [])
  return [preferences, update] as const
}

export function normalizePreferences(value: unknown): DesktopPreferences {
  const input = value && typeof value === 'object'
    ? value as Partial<DesktopPreferences>
    : {}
  return {
    panelDensity: input.panelDensity === 'compact' ? 'compact' : 'comfortable',
    conversationFontSize: clampFont(input.conversationFontSize),
    showAgentTopology: input.showAgentTopology !== false,
  }
}

function loadPreferences(): DesktopPreferences {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    return raw ? normalizePreferences(JSON.parse(raw) as unknown) : DEFAULT_DESKTOP_PREFERENCES
  } catch {
    return DEFAULT_DESKTOP_PREFERENCES
  }
}

function clampFont(value: unknown): number {
  if (typeof value !== 'number' || !Number.isFinite(value)) return 13
  return Math.min(18, Math.max(11, Math.round(value)))
}
