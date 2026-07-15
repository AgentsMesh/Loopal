import { describe, expect, it, vi } from 'vitest'
import { CommandRegistry } from './command-registry'

describe('CommandRegistry', () => {
  it('registers, lists, executes, and disposes commands', () => {
    const registry = new CommandRegistry()
    const second = registry.register({ id: 'b', title: 'Beta' }, () => 'b')
    const handler = vi.fn((_context, value) => value)
    const first = registry.register(
      { id: 'a', title: 'Alpha', category: 'Loopal', keybinding: 'Cmd+A' },
      handler,
    )
    expect(registry.list().map((command) => command.id)).toEqual(['a', 'b'])
    expect(registry.execute('a', { source: 'palette' }, 4)).toBe(4)
    expect(handler).toHaveBeenCalledWith({ source: 'palette' }, 4)
    expect(() => registry.register({ id: 'a', title: 'Again' }, vi.fn())).toThrow(
      'already registered',
    )
    first.dispose()
    first.dispose()
    expect(() => registry.execute('a', { source: 'api' })).toThrow('not found')
    second.dispose()
  })
})
