import { type CancellationToken, throwIfCancelled } from '../../../../base/common/cancellation'
import {
  DeleteGlobalSkillInputSchema,
  GetSkillInputSchema,
  PluginsResponseSchema,
  SkillDetailSchema,
  SkillsResponseSchema,
  UpsertGlobalSkillInputSchema,
  type DeleteGlobalSkillInput,
  type GetSkillInput,
  type PluginsResponse,
  type SkillDetail,
  type SkillsResponse,
  type UpsertGlobalSkillInput,
} from '../../../../shared/contracts'
import { type DesktopBackend } from '../../common/backend'
import { type CodeWorkbenchRuntimeRouter } from '../workspace/loopal-code-workbench'

export type LoopalSkillPluginOperations = Pick<DesktopBackend,
  'listSkills' | 'getSkill' | 'upsertGlobalSkill' | 'deleteGlobalSkill' | 'listPlugins'
>

export class LoopalSkillPluginService implements LoopalSkillPluginOperations {
  constructor(private readonly router: CodeWorkbenchRuntimeRouter) {}

  async listSkills(workspaceId: string, token: CancellationToken): Promise<SkillsResponse> {
    return SkillsResponseSchema.parse(await this.call(
      workspaceId, 'desktop/listSkills', { workspaceId }, token,
    ))
  }

  async getSkill(input: GetSkillInput, token: CancellationToken): Promise<SkillDetail> {
    const parsed = GetSkillInputSchema.parse(input)
    return SkillDetailSchema.parse(await this.call(
      parsed.workspaceId, 'desktop/getSkill', parsed, token,
    ))
  }

  async upsertGlobalSkill(
    input: UpsertGlobalSkillInput, token: CancellationToken,
  ): Promise<SkillDetail> {
    const parsed = UpsertGlobalSkillInputSchema.parse(input)
    return SkillDetailSchema.parse(await this.call(
      parsed.workspaceId, 'desktop/upsertSkill', parsed, token,
    ))
  }

  async deleteGlobalSkill(
    input: DeleteGlobalSkillInput, token: CancellationToken,
  ): Promise<SkillsResponse> {
    const parsed = DeleteGlobalSkillInputSchema.parse(input)
    return SkillsResponseSchema.parse(await this.call(
      parsed.workspaceId, 'desktop/deleteSkill', parsed, token,
    ))
  }

  async listPlugins(workspaceId: string, token: CancellationToken): Promise<PluginsResponse> {
    return PluginsResponseSchema.parse(await this.call(
      workspaceId, 'desktop/listPlugins', { workspaceId }, token,
    ))
  }

  private async call(
    workspaceId: string, method: string, input: unknown, token: CancellationToken,
  ): Promise<unknown> {
    throwIfCancelled(token)
    const runtime = await this.router.workspace(workspaceId)
    throwIfCancelled(token)
    const controller = new AbortController()
    const subscription = token.onCancellationRequested(() => controller.abort())
    try {
      const result = await runtime.host.request(method, input, controller.signal)
      throwIfCancelled(token)
      return result
    } finally {
      subscription.dispose()
    }
  }
}

export function bindLoopalSkillPlugins(
  service: LoopalSkillPluginService,
): LoopalSkillPluginOperations {
  return {
    listSkills: service.listSkills.bind(service),
    getSkill: service.getSkill.bind(service),
    upsertGlobalSkill: service.upsertGlobalSkill.bind(service),
    deleteGlobalSkill: service.deleteGlobalSkill.bind(service),
    listPlugins: service.listPlugins.bind(service),
  }
}
