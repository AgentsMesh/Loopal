import { describe, expect, it } from 'vitest'
import { activatePane, createDefaultLayout, findGroup, resolveActivePane } from './layout-model'

describe('LayoutModel', () => {
  it('creates the default editor and in-session panel split', () => {
    const layout = createDefaultLayout()
    expect(layout).toMatchObject({ type: 'split', direction: 'vertical', ratio: 0.72 })
    expect(findGroup(layout, 'editor')?.activePaneId).toBe('conversation')
    expect(findGroup(layout, 'editor')?.paneIds).toEqual(['conversation', 'federation'])
    expect(findGroup(layout, 'session')?.paneIds).not.toContain('settings')
    expect(findGroup(layout, 'editor')?.paneIds).not.toContain('diff')
    expect(findGroup(layout, 'session')?.paneIds).toContain('artifacts')
    expect(findGroup(layout, 'missing')).toBeUndefined()
  })

  it('activates only a pane contained by the requested group', () => {
    const layout = createDefaultLayout()
    const updated = activatePane(layout, 'session', 'agents')
    expect(findGroup(updated, 'session')?.activePaneId).toBe('agents')
    expect(activatePane(updated, 'session', 'missing')).toBeTruthy()
    expect(findGroup(activatePane(updated, 'missing', 'agents'), 'session')?.activePaneId).toBe(
      'agents',
    )
  })

  it('handles a root group', () => {
    const group = { type: 'group' as const, id: 'root', paneIds: ['one'], activePaneId: 'one' }
    expect(findGroup(group, 'root')).toBe(group)
    expect(resolveActivePane(group, 'root', 'fallback')).toBe('one')
    expect(activatePane(group, 'root', 'one')).toEqual(group)
    expect(activatePane(group, 'other', 'one')).toBe(group)
  })

  it('falls back when a group or its active pane is absent', () => {
    const group = { type: 'group' as const, id: 'root', paneIds: ['one'] }
    expect(resolveActivePane(group, 'root', 'fallback')).toBe('fallback')
    expect(resolveActivePane(group, 'missing', 'fallback')).toBe('fallback')
  })
})
