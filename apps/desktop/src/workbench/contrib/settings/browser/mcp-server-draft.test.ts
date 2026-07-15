import { describe, expect, it } from 'vitest'
import {
  editMcpServerDraft, mcpInputFromDraft, newMcpServerDraft, withSecretPatch,
} from './mcp-server-draft'

describe('MCP server draft', () => {
  it('preserves argument whitespace and explicit empty lines', () => {
    const input = mcpInputFromDraft({
      ...newMcpServerDraft(), name: 'test', command: 'node', argsText: '  padded  \n\n--flag=value ',
    })
    expect(input).toMatchObject({
      type: 'stdio', args: ['  padded  ', '', '--flag=value '],
    })
  })

  it('maps an entirely empty textarea to no arguments', () => {
    const input = mcpInputFromDraft({
      ...newMcpServerDraft(), name: 'test', command: 'node', argsText: '',
    })
    expect(input).toMatchObject({ type: 'stdio', args: [] })
  })

  it('edits stdio definitions with cwd and inherited-secret restrictions', () => {
    const draft = editMcpServerDraft({
      type: 'stdio', name: 'tools', source: 'global', command: 'node', args: ['a', ''],
      enabled: false, timeoutMs: 12_000, sharing: 'per-agent',
      cwdIsolation: { arg: '--profile', cacheSubdir: 'tools' },
      env: [{ name: 'TOKEN', configured: true }],
    })
    expect(draft).toMatchObject({
      lockedName: true, restrictedSecrets: true, argsText: 'a\n', cwdIsolation: true,
      cwdArg: '--profile', cacheSubdir: 'tools', secrets: [{ name: 'TOKEN' }],
    })
    const project = editMcpServerDraft({
      type: 'stdio', name: 'local', source: 'project', command: 'node', args: [],
      enabled: true, timeoutMs: 30_000, sharing: 'hub-singleton', cwdIsolation: null,
      env: [{ name: 'EMPTY', configured: false }],
    })
    expect(project).toMatchObject({
      restrictedSecrets: false, cwdIsolation: false,
      cwdArg: '--user-data-dir', cacheSubdir: '',
    })
  })

  it('edits and serializes HTTP definitions and optional cwd values', () => {
    const http = editMcpServerDraft({
      type: 'streamable-http', name: 'remote', source: 'plugin',
      url: 'https://example.test/mcp', enabled: true, timeoutMs: 10_000,
      sharing: 'spawn-tree', headers: [{ name: 'Authorization', configured: true }],
    })
    expect(http).toMatchObject({
      type: 'streamable-http', restrictedSecrets: true, command: '', argsText: '',
      cwdIsolation: false, secrets: [{ name: 'Authorization' }],
    })
    expect(mcpInputFromDraft(http)).toMatchObject({
      type: 'streamable-http', url: 'https://example.test/mcp',
    })
    const stdio = mcpInputFromDraft({
      ...newMcpServerDraft(), name: 'tools', command: 'node', cwdIsolation: true,
      cwdArg: '--profile', cacheSubdir: 'tools',
    })
    expect(stdio).toMatchObject({
      cwdIsolation: { arg: '--profile', cacheSubdir: 'tools' },
    })
  })

  it('adds, replaces, and removes write-only secret patches immutably', () => {
    const initial = {
      ...newMcpServerDraft(), secretPatches: [
        { target: 'env' as const, name: 'KEEP', operation: 'remove' as const },
      ],
    }
    const added = withSecretPatch(initial, {
      target: 'env', name: 'TOKEN', operation: 'set', value: 'one',
    }, 'TOKEN')
    expect(added.secretPatches).toHaveLength(2)
    const replaced = withSecretPatch(added, {
      target: 'env', name: 'TOKEN', operation: 'set', value: 'two',
    }, 'TOKEN')
    expect(replaced.secretPatches).toContainEqual(expect.objectContaining({ value: 'two' }))
    expect(replaced.secretPatches).not.toContainEqual(expect.objectContaining({ value: 'one' }))
    expect(withSecretPatch(replaced, undefined, 'TOKEN').secretPatches).toEqual([
      { target: 'env', name: 'KEEP', operation: 'remove' },
    ])
  })
})
