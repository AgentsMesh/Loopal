import { CancellationToken, CancellationTokenSource } from '../../../../base/common/cancellation'
import { type DesktopHostClient } from '../backend/loopal-backend-types'
import { LoopalSkillPluginService } from './loopal-skill-plugin-service'
import { type SessionRuntimeHandle } from '../runtime/session-runtime-registry'

const revision = 'a'.repeat(64)
const summary = {
  name: '/review', description: 'Review', hasArguments: true,
  source: 'global', scope: 'global', editable: true, effective: true, revision,
} as const
const detail = { workspaceId: 'workspace', ...summary, body: 'Review $ARGUMENTS' }

function harness() {
  const request = vi.fn<DesktopHostClient['request']>(async (method) => {
    if (method === 'desktop/listSkills' || method === 'desktop/deleteSkill') {
      return { workspaceId: 'workspace', skills: [summary] }
    }
    if (method === 'desktop/listPlugins') return { workspaceId: 'workspace', plugins: [] }
    return detail
  })
  const runtime = {
    workspaceId: 'workspace', sessionId: 'session', runtimeId: 'runtime', generation: 1,
    host: { request } as unknown as DesktopHostClient,
  } satisfies SessionRuntimeHandle
  const workspace = vi.fn(async () => runtime)
  return {
    service: new LoopalSkillPluginService({ workspace, liveSession: async () => runtime }),
    request,
  }
}

describe('LoopalSkillPluginService', () => {
  it('adapts all five operations to the workspace Host protocol', async () => {
    const { service, request } = harness()
    const token = CancellationToken.None
    await service.listSkills('workspace', token)
    await service.getSkill({ workspaceId: 'workspace', name: '/review' }, token)
    await service.upsertGlobalSkill({
      workspaceId: 'workspace', name: '/review', description: 'Review',
      body: 'Review $ARGUMENTS', expectedRevision: revision,
    }, token)
    await service.deleteGlobalSkill({
      workspaceId: 'workspace', name: '/review', expectedRevision: revision,
    }, token)
    await service.listPlugins('workspace', token)
    expect(request.mock.calls.map(([method]) => method)).toEqual([
      'desktop/listSkills', 'desktop/getSkill', 'desktop/upsertSkill',
      'desktop/deleteSkill', 'desktop/listPlugins',
    ])
  })

  it('validates requests and aborts an in-flight Host request', async () => {
    const invalid = harness()
    await expect(invalid.service.getSkill({
      workspaceId: 'workspace', name: '../bad',
    }, CancellationToken.None)).rejects.toThrow()
    expect(invalid.request).not.toHaveBeenCalled()

    const pending = harness()
    pending.request.mockImplementationOnce(async (_method, _input, signal) => new Promise(
      (_resolve, reject) => signal?.addEventListener('abort', () => reject(new Error('aborted'))),
    ))
    const source = new CancellationTokenSource()
    const result = pending.service.listSkills('workspace', source.token)
    await vi.waitFor(() => expect(pending.request).toHaveBeenCalled())
    source.cancel()
    await expect(result).rejects.toThrow('aborted')
    source.dispose()
  })
})
