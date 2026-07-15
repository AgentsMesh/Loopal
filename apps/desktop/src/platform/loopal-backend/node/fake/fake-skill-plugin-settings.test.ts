import { CancellationToken } from '../../../../base/common/cancellation'
import { bindFakeSkillPlugins } from './fake-skill-plugin-settings'

describe('fake Skill and Plugin settings', () => {
  it('models duplicate definitions, create-only writes, CAS updates, and deletes', async () => {
    const service = bindFakeSkillPlugins('workspace')
    const initial = await service.listSkills('workspace', CancellationToken.None)
    const commits = initial.skills.filter((skill) => skill.name === '/commit')
    expect(commits).toHaveLength(2)
    expect(commits.find((skill) => skill.scope === 'global')?.effective).toBe(false)
    expect(commits.find((skill) => skill.scope === 'project')?.effective).toBe(true)

    const created = await service.upsertGlobalSkill({
      workspaceId: 'workspace', name: '/ship', description: 'Ship', body: 'Ship $ARGUMENTS',
    }, CancellationToken.None)
    expect(created).toMatchObject({ name: '/ship', effective: true, hasArguments: true })
    await expect(service.upsertGlobalSkill({
      workspaceId: 'workspace', name: '/ship', description: 'Overwrite', body: 'unsafe',
    }, CancellationToken.None)).rejects.toThrow('changed on disk')
    const updated = await service.upsertGlobalSkill({
      workspaceId: 'workspace', name: '/ship', description: 'Ship safely', body: 'Ship',
      expectedRevision: created.revision,
    }, CancellationToken.None)
    await expect(service.deleteGlobalSkill({
      workspaceId: 'workspace', name: '/ship', expectedRevision: created.revision,
    }, CancellationToken.None)).rejects.toThrow('changed on disk')
    const removed = await service.deleteGlobalSkill({
      workspaceId: 'workspace', name: '/ship', expectedRevision: updated.revision,
    }, CancellationToken.None)
    expect(removed.skills.some((skill) => skill.name === '/ship')).toBe(false)
    await expect(service.getSkill({
      workspaceId: 'workspace', name: '/audit',
    }, CancellationToken.None)).rejects.toThrow('Global skill not found')
  })

  it('lists Plugin contributions without offering mutation', async () => {
    const service = bindFakeSkillPlugins('workspace')
    await expect(service.listPlugins('workspace', CancellationToken.None)).resolves.toEqual({
      workspaceId: 'workspace',
      plugins: [expect.objectContaining({
        name: 'quality', source: 'plugin:quality', skills: ['/audit'],
        mcpServers: ['reviewer'], hookCount: 1,
      })],
    })
  })
})
