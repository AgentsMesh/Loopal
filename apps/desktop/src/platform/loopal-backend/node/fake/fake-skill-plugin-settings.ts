import { CancellationToken, throwIfCancelled } from '../../../../base/common/cancellation'
import {
  DeleteGlobalSkillInputSchema,
  GetSkillInputSchema,
  UpsertGlobalSkillInputSchema,
  type PluginSummary,
  type SkillDetail,
  type SkillSummary,
  type SkillsResponse,
} from '../../../../shared/contracts'
import { type LoopalSkillPluginOperations } from '../settings/loopal-skill-plugin-service'

export function bindFakeSkillPlugins(workspaceId: string): LoopalSkillPluginOperations {
  let sequence = 2
  const globals = new Map<string, SkillDetail>([[
    '/commit', detail(workspaceId, '/commit', 'Create a focused commit', 'Commit $ARGUMENTS', 1),
  ]])
  const readonly: SkillSummary[] = [{
    name: '/audit', description: 'Audit the current workspace', hasArguments: true,
    source: 'plugin:quality', scope: 'plugin', editable: false, effective: true,
  }, {
    name: '/commit', description: 'Project commit policy', hasArguments: true,
    source: 'project', scope: 'project', editable: false, effective: true,
  }]
  const plugins: PluginSummary[] = [{
    name: 'quality', source: 'plugin:quality', skills: ['/audit'], mcpServers: ['reviewer'],
    hookCount: 1, hasSettings: true, hasInstructions: true, hasMemory: false,
  }]
  const requireWorkspace = (value: string): void => {
    if (value !== workspaceId) throw new Error(`Unknown workspace: ${value}`)
  }
  const list = (): SkillsResponse => ({
    workspaceId,
    skills: [...globals.values()].map((entry) => summary(entry, readonly))
      .concat(structuredClone(readonly)),
  })
  return {
    listSkills: async (input, token = CancellationToken.None) => {
      throwIfCancelled(token); requireWorkspace(input); return list()
    },
    getSkill: async (input, token = CancellationToken.None) => {
      throwIfCancelled(token)
      const parsed = GetSkillInputSchema.parse(input); requireWorkspace(parsed.workspaceId)
      const skill = globals.get(parsed.name)
      if (!skill) throw new Error(`Global skill not found: ${parsed.name}`)
      return structuredClone(skill)
    },
    upsertGlobalSkill: async (input, token = CancellationToken.None) => {
      throwIfCancelled(token)
      const parsed = UpsertGlobalSkillInputSchema.parse(input); requireWorkspace(parsed.workspaceId)
      const current = globals.get(parsed.name)
      if (current && parsed.expectedRevision !== current.revision) throw conflict(parsed.name)
      if (!current && parsed.expectedRevision !== undefined) throw conflict(parsed.name)
      const next = detail(
        workspaceId, parsed.name, parsed.description, parsed.body, sequence++, readonly,
      )
      globals.set(parsed.name, next)
      return structuredClone(next)
    },
    deleteGlobalSkill: async (input, token = CancellationToken.None) => {
      throwIfCancelled(token)
      const parsed = DeleteGlobalSkillInputSchema.parse(input); requireWorkspace(parsed.workspaceId)
      const current = globals.get(parsed.name)
      if (!current || current.revision !== parsed.expectedRevision) throw conflict(parsed.name)
      globals.delete(parsed.name)
      return list()
    },
    listPlugins: async (input, token = CancellationToken.None) => {
      throwIfCancelled(token); requireWorkspace(input)
      return { workspaceId, plugins: structuredClone(plugins) }
    },
  }
}

export function bindUnavailableSkillPlugins(reason: string): LoopalSkillPluginOperations {
  const fail = (token: CancellationToken): never => {
    throwIfCancelled(token); throw new Error(reason)
  }
  return {
    listSkills: async (_workspaceId, token) => fail(token),
    getSkill: async (_input, token) => fail(token),
    upsertGlobalSkill: async (_input, token) => fail(token),
    deleteGlobalSkill: async (_input, token) => fail(token),
    listPlugins: async (_workspaceId, token) => fail(token),
  }
}

function detail(
  workspaceId: string, name: string, description: string, body: string, revision: number,
  inherited: readonly SkillSummary[] = [],
): SkillDetail {
  return {
    workspaceId, name, description, body, hasArguments: body.includes('$ARGUMENTS'),
    source: 'global', scope: 'global', editable: true,
    effective: !inherited.some((skill) => skill.name === name && skill.effective),
    revision: revision.toString(16).padStart(64, '0'),
  }
}

function summary(skill: SkillDetail, inherited: readonly SkillSummary[]): SkillSummary {
  const { workspaceId: _workspaceId, body: _body, ...value } = skill
  return { ...value, effective: !inherited.some((item) => item.name === skill.name && item.effective) }
}

function conflict(name: string): Error {
  return new Error(`Skill changed on disk; reload before editing: ${name}`)
}
