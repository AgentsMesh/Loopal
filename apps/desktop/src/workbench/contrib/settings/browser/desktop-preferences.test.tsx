import { act, renderHook } from '@testing-library/react'
import {
  DEFAULT_DESKTOP_PREFERENCES, normalizePreferences, useDesktopPreferences,
} from './desktop-preferences'

describe('desktop preferences', () => {
  beforeEach(() => localStorage.clear())

  it('normalizes desktop values at their supported bounds', () => {
    expect(normalizePreferences(null)).toEqual(DEFAULT_DESKTOP_PREFERENCES)
    expect(normalizePreferences({
      panelDensity: 'compact', conversationFontSize: 99, showAgentTopology: false,
    })).toEqual({
      panelDensity: 'compact', conversationFontSize: 18, showAgentTopology: false,
    })
    expect(normalizePreferences({ conversationFontSize: Number.NaN })).toEqual(
      DEFAULT_DESKTOP_PREFERENCES,
    )
  })

  it('persists updates and reloads them in a fresh hook', () => {
    const first = renderHook(useDesktopPreferences)
    act(() => first.result.current[1]({ panelDensity: 'compact', conversationFontSize: 15 }))
    expect(first.result.current[0]).toMatchObject({ panelDensity: 'compact', conversationFontSize: 15 })
    first.unmount()
    const second = renderHook(useDesktopPreferences)
    expect(second.result.current[0]).toMatchObject({ panelDensity: 'compact', conversationFontSize: 15 })
  })

  it('falls back when storage reads or writes are unavailable', () => {
    const get = vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => { throw new Error() })
    const first = renderHook(useDesktopPreferences)
    expect(first.result.current[0]).toEqual(DEFAULT_DESKTOP_PREFERENCES)
    get.mockRestore()
    vi.spyOn(Storage.prototype, 'setItem').mockImplementationOnce(() => { throw new Error() })
    act(() => first.result.current[1]({ conversationFontSize: 10 }))
    expect(first.result.current[0].conversationFontSize).toBe(11)
    localStorage.setItem('loopal.desktop.preferences.v1', '{broken')
    expect(renderHook(useDesktopPreferences).result.current[0]).toEqual(DEFAULT_DESKTOP_PREFERENCES)
  })
})
