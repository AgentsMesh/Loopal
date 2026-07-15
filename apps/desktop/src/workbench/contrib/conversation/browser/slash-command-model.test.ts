import { describe, expect, it } from 'vitest'
import {
  BUILTIN_SLASH_COMMANDS, BUILTIN_SLASH_NAMES, parseSlashInput,
} from './slash-command-model'

describe('slash command parser', () => {
  it('keeps ordinary, escaped, Skill, and unknown slash input on the message plane', () => {
    for (const value of [
      'hello', '//plan', '/desktop-check alpha', '/unknown', '  /workspace-skill',
    ]) expect(parseSlashInput(value)).toEqual({ kind: 'message' })
  })

  it.each([
    ['/act', { type: 'mode', mode: 'act' }],
    ['/plan', { type: 'mode', mode: 'plan' }],
    ['/clear', { type: 'clear' }],
    ['/compact', { type: 'compact' }],
    ['/compact preserve decisions', { type: 'compact', instructions: 'preserve decisions' }],
    ['/model claude-opus', { type: 'model', model: 'claude-opus' }],
    ['/rewind 0', { type: 'rewind', turnIndex: 0 }],
    ['/permission bypass', { type: 'permission', mode: 'bypass' }],
    ['/permission ask_dangerous', { type: 'permission', mode: 'ask_dangerous' }],
    ['/decision classifier', { type: 'decision', mode: 'classifier' }],
    ['/sandbox read_only', { type: 'sandbox', policy: 'read_only' }],
    ['/suspend', { type: 'suspend' }],
    ['/unsuspend', { type: 'unsuspend' }],
    ['/mcp', { type: 'mcp_status' }],
    ['/mcp status', { type: 'mcp_status' }],
    ['/mcp reconnect github', { type: 'mcp_reconnect', server: 'github' }],
    ['/mcp disconnect github', { type: 'mcp_disconnect', server: 'github' }],
  ])('maps %s to the typed Runtime control', (input, command) => {
    expect(parseSlashInput(input)).toEqual({ kind: 'control', command })
  })

  it('opens local help without creating a Runtime control', () => {
    expect(parseSlashInput('/help')).toEqual({ kind: 'help', query: '' })
    expect(parseSlashInput('/help /permission')).toEqual({
      kind: 'help', query: 'permission',
    })
  })

  it.each([
    ['/act now', 'unexpected_arguments'],
    ['/model', 'required_argument'],
    ['/rewind -1', 'invalid_value'],
    ['/rewind 1.5', 'invalid_value'],
    ['/rewind 99999999999999999999', 'invalid_value'],
    ['/permission unrestricted', 'invalid_value'],
    ['/decision automatic', 'invalid_value'],
    ['/sandbox write_all', 'invalid_value'],
    ['/mcp reconnect', 'invalid_value'],
    ['/mcp restart github', 'invalid_value'],
  ])('rejects invalid local parameters for %s', (input, code) => {
    expect(parseSlashInput(input)).toMatchObject({ kind: 'error', code })
  })

  it('exposes one canonical static entry for every built-in', () => {
    expect(BUILTIN_SLASH_COMMANDS).toHaveLength(13)
    expect(BUILTIN_SLASH_NAMES.size).toBe(BUILTIN_SLASH_COMMANDS.length)
  })
})
