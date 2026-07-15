import { describe, expect, it } from 'vitest'
import { PaneRegistry } from './pane-registry'

describe('PaneRegistry', () => {
  it('registers, orders, filters, and disposes panes', () => {
    const registry = new PaneRegistry()
    const later = registry.register({
      id: 'later', kind: 'diagnostics', title: 'Later', location: 'session', order: 2,
    })
    const first = registry.register({
      id: 'first', kind: 'agents', title: 'First', location: 'session', order: 1,
    })
    registry.register({
      id: 'editor', kind: 'conversation', title: 'Editor', location: 'editor', order: 0,
    })
    expect(registry.list('session').map((pane) => pane.id)).toEqual(['first', 'later'])
    expect(registry.list()).toHaveLength(3)
    expect(registry.get('first')?.title).toBe('First')
    expect(() => registry.register({
      id: 'first', kind: 'tasks', title: 'Duplicate', location: 'panel', order: 0,
    })).toThrow('already registered')
    first.dispose()
    first.dispose()
    expect(registry.get('first')).toBeUndefined()
    later.dispose()
  })
})
