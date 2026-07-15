import { describe, expect, it, vi } from 'vitest'
import { CancellationToken, CancellationTokenSource } from '../../../../base/common/cancellation'
import { type DesktopHostClient } from '../backend/loopal-backend-types'
import { LoopalSettingsService } from './loopal-settings-service'
import { type SessionRuntimeHandle } from '../runtime/session-runtime-registry'

const values = {
  model: 'gpt-5', modelRouting: {
    default: '', summarization: '', classification: '', refine: '',
  }, permissionMode: 'bypass' as const, decisionMode: 'manual' as const,
  sandboxPolicy: 'default_write' as const, thinking: { type: 'auto' as const },
  maxContextTokens: 0, memoryEnabled: true, microcompactIdleMinutes: 60,
  telemetryEnabled: true, outputStyle: '',
}

function harness(result: unknown = {
  workspaceId: 'workspace', settings: values, configuredProviders: [],
  providers: {
    anthropic: emptyProvider(), openai: emptyProvider(), google: emptyProvider(),
  },
  openaiCompatible: [],
  resolvedEntries: [{ key: 'model', value: 'gpt-5' }], settingSources: ['defaults'],
}) {
  const request = vi.fn<DesktopHostClient['request']>(async () => result)
  const runtime = {
    workspaceId: 'workspace', sessionId: 'session', runtimeId: 'runtime', generation: 1,
    host: { request } as unknown as DesktopHostClient,
  } satisfies SessionRuntimeHandle
  const workspace = vi.fn(async () => runtime)
  const service = new LoopalSettingsService({
    workspace,
    liveSession: async () => runtime,
  })
  return { service, request, workspace }
}

describe('LoopalSettingsService', () => {
  it('routes typed reads and atomic updates through the workspace leader', async () => {
    const { service, request } = harness()
    await expect(service.getLoopalSettings('workspace', CancellationToken.None)).resolves
      .toMatchObject({ settings: { model: 'gpt-5' } })
    await service.updateLoopalSettings({ workspaceId: 'workspace', settings: values }, CancellationToken.None)
    expect(request.mock.calls.map(([method, input]) => [method, input])).toEqual([
      ['desktop/getSettings', { workspaceId: 'workspace' }],
      ['desktop/updateSettings', { workspaceId: 'workspace', settings: values }],
    ])
  })

  it('honors cancellation and rejects secret-bearing responses', async () => {
    const cancelled = harness()
    await expect(cancelled.service.getLoopalSettings(
      'workspace', CancellationToken.Cancelled,
    )).rejects.toThrow('cancelled')
    expect(cancelled.request).not.toHaveBeenCalled()

    const pending = harness()
    pending.request.mockImplementationOnce(async (_method, _params, signal) => new Promise(
      (_resolve, reject) => signal?.addEventListener('abort', () => reject(new Error('aborted'))),
    ))
    const source = new CancellationTokenSource()
    const request = pending.service.getLoopalSettings('workspace', source.token)
    await vi.waitFor(() => expect(pending.request).toHaveBeenCalled())
    source.cancel()
    await expect(request).rejects.toThrow('aborted')
    source.dispose()

    const unsafe = harness({
      workspaceId: 'workspace', settings: values, configuredProviders: [],
      providers: {
        anthropic: emptyProvider(), openai: emptyProvider(), google: emptyProvider(),
      },
      openaiCompatible: [],
      resolvedEntries: [], settingSources: ['defaults'], apiKey: 'secret',
    })
    await expect(unsafe.service.getLoopalSettings(
      'workspace', CancellationToken.None,
    )).rejects.toThrow()
  })
})

function emptyProvider() {
  return { enabled: false, baseUrl: '', apiKeyEnv: '', apiKeyConfigured: false }
}
