import { CancellationToken } from '../../../../base/common/cancellation'
import { createBackendStub } from '../../../../../test/support/backend/backend-stub'
import { callSkillPluginBackend } from './skill-plugin-channel'

const revision = 'a'.repeat(64)

describe('Skill Plugin backend channel', () => {
  it('dispatches and validates every explicit operation', async () => {
    const backend = createBackendStub()
    const token = CancellationToken.None
    await expect(callSkillPluginBackend(
      backend, 'listSkills', { workspaceId: 'workspace' }, token,
    )).resolves.toMatchObject({ handled: true, value: { workspaceId: 'workspace' } })
    await callSkillPluginBackend(
      backend, 'getSkill', { workspaceId: 'workspace', name: '/review' }, token,
    )
    await callSkillPluginBackend(backend, 'upsertGlobalSkill', {
      workspaceId: 'workspace', name: '/review', description: 'Review', body: 'Review',
    }, token)
    await callSkillPluginBackend(backend, 'deleteGlobalSkill', {
      workspaceId: 'workspace', name: '/review', expectedRevision: revision,
    }, token)
    await callSkillPluginBackend(
      backend, 'listPlugins', { workspaceId: 'workspace' }, token,
    )
    expect(backend.getSkill).toHaveBeenCalledWith({
      workspaceId: 'workspace', name: '/review',
    }, token)
    expect(backend.upsertGlobalSkill).toHaveBeenCalledWith({
      workspaceId: 'workspace', name: '/review', description: 'Review', body: 'Review',
    }, token)
    expect(backend.deleteGlobalSkill).toHaveBeenCalledWith({
      workspaceId: 'workspace', name: '/review', expectedRevision: revision,
    }, token)
  })

  it('rejects path-like names, unknown fields, and invalid revisions', async () => {
    const backend = createBackendStub()
    const token = CancellationToken.None
    await expect(callSkillPluginBackend(backend, 'getSkill', {
      workspaceId: 'workspace', name: '/../escape',
    }, token)).rejects.toThrow()
    await expect(callSkillPluginBackend(backend, 'upsertGlobalSkill', {
      workspaceId: 'workspace', name: '/review', description: 'Review', body: 'Review',
      path: '/tmp/escape',
    }, token)).rejects.toThrow()
    await expect(callSkillPluginBackend(backend, 'deleteGlobalSkill', {
      workspaceId: 'workspace', name: '/review', expectedRevision: 'stale',
    }, token)).rejects.toThrow()
    await expect(callSkillPluginBackend(
      backend, 'unknown', {}, token,
    )).resolves.toEqual({ handled: false })
  })
})
