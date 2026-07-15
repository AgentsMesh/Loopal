import {
  PluginsResponseSchema,
  SkillDetailSchema,
  SkillsResponseSchema,
  type DeleteGlobalSkillInput,
  type GetSkillInput,
  type PluginsResponse,
  type SkillDetail,
  type SkillsResponse,
  type UpsertGlobalSkillInput,
} from '../../../../shared/contracts'
import { type ChannelClient } from '../../../ipc/common/channel'

export interface SkillPluginClientOperations {
  listSkills(workspaceId: string): Promise<SkillsResponse>
  getSkill(input: GetSkillInput): Promise<SkillDetail>
  upsertGlobalSkill(input: UpsertGlobalSkillInput): Promise<SkillDetail>
  deleteGlobalSkill(input: DeleteGlobalSkillInput): Promise<SkillsResponse>
  listPlugins(workspaceId: string): Promise<PluginsResponse>
}

export function bindSkillPluginClient(client: ChannelClient): SkillPluginClientOperations {
  return {
    listSkills: async (workspaceId) => SkillsResponseSchema.parse(
      await client.call('desktopBackend', 'listSkills', { workspaceId }),
    ),
    getSkill: async (input) => SkillDetailSchema.parse(
      await client.call('desktopBackend', 'getSkill', input),
    ),
    upsertGlobalSkill: async (input) => SkillDetailSchema.parse(
      await client.call('desktopBackend', 'upsertGlobalSkill', input),
    ),
    deleteGlobalSkill: async (input) => SkillsResponseSchema.parse(
      await client.call('desktopBackend', 'deleteGlobalSkill', input),
    ),
    listPlugins: async (workspaceId) => PluginsResponseSchema.parse(
      await client.call('desktopBackend', 'listPlugins', { workspaceId }),
    ),
  }
}
