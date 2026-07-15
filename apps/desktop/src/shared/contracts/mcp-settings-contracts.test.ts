import { describe, expect, it } from 'vitest'
import {
  McpServersResponseSchema, UpsertMcpServerInputSchema,
} from './mcp-settings-contracts'

const http = {
  workspaceId: 'workspace',
  server: {
    type: 'streamable-http', name: 'remote', url: 'https://example.test/mcp',
    enabled: true, timeoutMs: 30_000, sharing: 'hub-singleton', secretPatches: [],
  },
}

describe('MCP Settings contracts', () => {
  it('accepts typed stdio and public HTTP definitions', () => {
    expect(UpsertMcpServerInputSchema.parse(http)).toEqual(http)
    expect(UpsertMcpServerInputSchema.parse({
      workspaceId: 'workspace', server: {
        type: 'stdio', name: 'local', command: 'node', args: ['server.js'],
        enabled: false, timeoutMs: 100, sharing: 'per-agent', cwdIsolation: null,
        secretPatches: [{ target: 'env', name: 'TOKEN', operation: 'remove' }],
      },
    }).server.type).toBe('stdio')
  })

  it('rejects URL credentials, queries, invalid secret targets, and response values', () => {
    for (const url of [
      'file:///tmp/mcp', 'https://user:secret@example.test/mcp',
      'https://example.test/mcp?token=secret', 'https://example.test/mcp#secret',
    ]) {
      expect(() => UpsertMcpServerInputSchema.parse({
        ...http, server: { ...http.server, url },
      })).toThrow()
    }
    expect(() => UpsertMcpServerInputSchema.parse({
      ...http, server: { ...http.server, secretPatches: [{
        target: 'env', name: 'TOKEN', operation: 'set', value: 'secret',
      }] },
    })).toThrow()
    expect(() => UpsertMcpServerInputSchema.parse({
      ...http, server: { ...http.server, secretPatches: [
        { target: 'header', name: 'Authorization', operation: 'remove' },
        { target: 'header', name: 'authorization', operation: 'remove' },
      ] },
    })).toThrow()
    expect(() => McpServersResponseSchema.parse({
      workspaceId: 'workspace', servers: [{
        type: 'streamable-http', name: 'remote', source: 'local',
        url: 'https://example.test/mcp', enabled: true, timeoutMs: 30_000,
        sharing: 'hub-singleton', headers: [{
          name: 'Authorization', configured: true, value: 'must-not-cross',
        }],
      }],
    })).toThrow()
  })
})
