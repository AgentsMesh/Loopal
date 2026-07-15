import { type CancellationToken } from '../../../../base/common/cancellation'
import {
  DeleteGlobalSkillInputSchema,
  GetSkillInputSchema,
  ListSkillPluginsInputSchema,
  PluginsResponseSchema,
  SkillDetailSchema,
  SkillsResponseSchema,
  UpsertGlobalSkillInputSchema,
} from '../../../../shared/contracts'
import { type DesktopBackend } from '../backend'

export async function callSkillPluginBackend(
  backend: DesktopBackend,
  command: string,
  arg: unknown,
  token: CancellationToken,
): Promise<{ handled: false } | { handled: true; value: unknown }> {
  switch (command) {
    case 'listSkills': {
      const { workspaceId } = ListSkillPluginsInputSchema.parse(arg)
      return handled(SkillsResponseSchema.parse(await backend.listSkills(workspaceId, token)))
    }
    case 'getSkill': {
      const input = GetSkillInputSchema.parse(arg)
      return handled(SkillDetailSchema.parse(await backend.getSkill(input, token)))
    }
    case 'upsertGlobalSkill': {
      const input = UpsertGlobalSkillInputSchema.parse(arg)
      return handled(SkillDetailSchema.parse(await backend.upsertGlobalSkill(input, token)))
    }
    case 'deleteGlobalSkill': {
      const input = DeleteGlobalSkillInputSchema.parse(arg)
      return handled(SkillsResponseSchema.parse(await backend.deleteGlobalSkill(input, token)))
    }
    case 'listPlugins': {
      const { workspaceId } = ListSkillPluginsInputSchema.parse(arg)
      return handled(PluginsResponseSchema.parse(await backend.listPlugins(workspaceId, token)))
    }
    default:
      return { handled: false }
  }
}

function handled(value: unknown): { handled: true; value: unknown } {
  return { handled: true, value }
}
