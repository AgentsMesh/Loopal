import { describe, expect, it, vi } from 'vitest'
import { CancellationToken } from '../../../../base/common/cancellation'
import { type DesktopHostClient } from '../backend/loopal-backend-types'
import { LoopalMcpSettingsService } from './loopal-mcp-settings-service'
import { type SessionRuntimeHandle } from '../runtime/session-runtime-registry'

function harness(result: unknown = { workspaceId: 'workspace', servers: [] }) {
  const request = vi.fn<DesktopHostClient['request']>(async () => result)
  const runtime = {
    workspaceId: 'workspace', sessionId: 'session', runtimeId: 'runtime', generation: 1,
    host: { request } as unknown as DesktopHostClient,
  } satisfies SessionRuntimeHandle
  const service = new LoopalMcpSettingsService({
    workspace: async () => runtime, liveSession: async () => runtime,
  })
  return { service, request }
}

describe('LoopalMcpSettingsService', () => {
  it('routes all typed methods through the workspace leader', async () => {
    const { service, request } = harness()
    const token = CancellationToken.None
    await service.listMcpServers('workspace', token)
    const server = {
      type: 'stdio' as const, name: 'local', command: 'node', args: [], enabled: true,
      timeoutMs: 30_000, sharing: 'hub-singleton' as const, cwdIsolation: null,
      secretPatches: [],
    }
    await service.upsertMcpServer({ workspaceId: 'workspace', server }, token)
    await service.deleteMcpServer({ workspaceId: 'workspace', name: 'local' }, token)
    expect(request.mock.calls.map(([method, input]) => [method, input])).toEqual([
      ['desktop/listMcpServers', { workspaceId: 'workspace' }],
      ['desktop/upsertMcpServer', { workspaceId: 'workspace', server }],
      ['desktop/deleteMcpServer', { workspaceId: 'workspace', name: 'local' }],
    ])
  })

  it('rejects secret-bearing host responses', async () => {
    const { service } = harness({
      workspaceId: 'workspace', servers: [{
        type: 'stdio', name: 'local', source: 'local', command: 'node', args: [],
        enabled: true, timeoutMs: 30_000, sharing: 'hub-singleton', cwdIsolation: null,
        env: [{ name: 'TOKEN', configured: true, value: 'must-not-cross' }],
      }],
    })
    await expect(service.listMcpServers('workspace', CancellationToken.None)).rejects.toThrow()
  })
})
