import { Emitter } from '../../../../base/common/event'
import { type ChannelClient } from '../../../ipc/common/channel'
import { bindSkillPluginClient } from './skill-plugin-client'

const revision = 'a'.repeat(64)

describe('Skill Plugin client', () => {
  it('uses the Desktop channel and validates untrusted responses', async () => {
    const call = vi.fn(async (_channel: string, command: string) => {
      if (command === 'listSkills' || command === 'deleteGlobalSkill') {
        return { workspaceId: 'workspace', skills: [] }
      }
      if (command === 'listPlugins') return { workspaceId: 'workspace', plugins: [] }
      return {
        workspaceId: 'workspace', name: '/review', description: 'Review', body: 'Review',
        hasArguments: false, source: 'global', scope: 'global', editable: true,
        effective: true, revision,
      }
    })
    const events = new Emitter<unknown>()
    const client = {
      call, listen: () => events.event, dispose: () => events.dispose(),
    } as unknown as ChannelClient
    const service = bindSkillPluginClient(client)
    await service.listSkills('workspace')
    await service.getSkill({ workspaceId: 'workspace', name: '/review' })
    await service.upsertGlobalSkill({
      workspaceId: 'workspace', name: '/review', description: 'Review', body: 'Review',
      expectedRevision: revision,
    })
    await service.deleteGlobalSkill({
      workspaceId: 'workspace', name: '/review', expectedRevision: revision,
    })
    await service.listPlugins('workspace')
    expect(call.mock.calls.map(([, command]) => command)).toEqual([
      'listSkills', 'getSkill', 'upsertGlobalSkill', 'deleteGlobalSkill', 'listPlugins',
    ])
  })

  it('rejects malformed Host output before it reaches the renderer', async () => {
    const client = {
      call: vi.fn(async () => ({ workspaceId: 'workspace', skills: [{ path: '/secret' }] })),
    } as unknown as ChannelClient
    await expect(bindSkillPluginClient(client).listSkills('workspace')).rejects.toThrow()
  })
})
