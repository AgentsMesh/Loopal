import { describe, expect, it } from 'vitest'
import { CancellationToken } from '../../../../base/common/cancellation'
import { bindFakeMcpSettings, bindUnavailableMcpSettings } from './fake-mcp-settings'

describe('fake MCP Settings', () => {
  it('models secret status and disabled definitions without retaining values', async () => {
    const service = bindFakeMcpSettings('workspace')
    const token = CancellationToken.None
    const server = {
      type: 'stdio' as const, name: 'local', command: 'node', args: [], enabled: false,
      timeoutMs: 30_000, sharing: 'hub-singleton' as const, cwdIsolation: null,
      secretPatches: [{
        target: 'env' as const, name: 'TOKEN', operation: 'set' as const, value: 'secret',
      }],
    }
    const created = await service.upsertMcpServer({ workspaceId: 'workspace', server }, token)
    expect(created.servers[0]).toMatchObject({ enabled: false, env: [{
      name: 'TOKEN', configured: true,
    }] })
    expect(JSON.stringify(created)).not.toContain('secret')
    const removed = await service.upsertMcpServer({
      workspaceId: 'workspace', server: {
        ...server, enabled: true,
        secretPatches: [{ target: 'env', name: 'TOKEN', operation: 'remove' }],
      },
    }, token)
    expect(removed.servers[0]).toMatchObject({ enabled: true, env: [] })
    await expect(service.deleteMcpServer({
      workspaceId: 'outside', name: 'local',
    }, token)).rejects.toThrow('Unknown workspace')
    await expect(service.deleteMcpServer({
      workspaceId: 'workspace', name: 'local',
    }, token)).resolves.toMatchObject({ servers: [] })
  })

  it('models HTTP headers, immutable reads, and unavailable operations', async () => {
    const service = bindFakeMcpSettings('workspace')
    const server = {
      type: 'streamable-http' as const, name: 'remote', url: 'https://mcp.example.test/api',
      enabled: true, timeoutMs: 5_000, sharing: 'per-agent' as const,
      secretPatches: [{
        target: 'header' as const, name: 'Authorization', operation: 'set' as const,
        value: 'Bearer private',
      }],
    }
    const created = await service.upsertMcpServer(
      { workspaceId: 'workspace', server }, CancellationToken.None,
    )
    expect(created.servers[0]).toMatchObject({
      type: 'streamable-http', source: 'local',
      headers: [{ name: 'Authorization', configured: true }],
    })
    expect(JSON.stringify(created)).not.toContain('Bearer private')

    created.servers.length = 0
    expect(await service.listMcpServers('workspace', CancellationToken.None))
      .toHaveProperty('servers.length', 1)
    const updated = await service.upsertMcpServer({
      workspaceId: 'workspace', server: {
        ...server,
        secretPatches: [
          { target: 'header', name: 'Authorization', operation: 'remove' },
          { target: 'header', name: 'X-Token', operation: 'set', value: 'replacement' },
        ],
      },
    }, CancellationToken.None)
    expect(updated.servers[0]).toMatchObject({
      headers: [{ name: 'X-Token', configured: true }],
    })
    await expect(service.listMcpServers('other', CancellationToken.None))
      .rejects.toThrow('Unknown workspace')

    const unavailable = bindUnavailableMcpSettings('settings offline')
    await expect(unavailable.listMcpServers('workspace', CancellationToken.None))
      .rejects.toThrow('settings offline')
    await expect(unavailable.upsertMcpServer(
      { workspaceId: 'workspace', server }, CancellationToken.Cancelled,
    )).rejects.toThrow('cancelled')
    await expect(unavailable.deleteMcpServer(
      { workspaceId: 'workspace', name: 'remote' }, CancellationToken.None,
    )).rejects.toThrow('settings offline')
  })
})
