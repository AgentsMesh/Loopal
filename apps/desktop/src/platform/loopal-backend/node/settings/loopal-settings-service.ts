import { type CancellationToken, throwIfCancelled } from '../../../../base/common/cancellation'
import {
  LoopalDefaultSettingsSchema,
  type LoopalDefaultSettings,
  type UpdateLoopalSettingsInput,
} from '../../../../shared/contracts'
import { type DesktopBackend } from '../../common/backend'
import { type CodeWorkbenchRuntimeRouter } from '../workspace/loopal-code-workbench'

export type LoopalSettingsOperations = Pick<
  DesktopBackend, 'getLoopalSettings' | 'updateLoopalSettings'
>

export class LoopalSettingsService implements LoopalSettingsOperations {
  constructor(private readonly router: CodeWorkbenchRuntimeRouter) {}

  async getLoopalSettings(
    workspaceId: string,
    token: CancellationToken,
  ): Promise<LoopalDefaultSettings> {
    return LoopalDefaultSettingsSchema.parse(await this.call(
      workspaceId, 'desktop/getSettings', { workspaceId }, token,
    ))
  }

  async updateLoopalSettings(
    input: UpdateLoopalSettingsInput,
    token: CancellationToken,
  ): Promise<LoopalDefaultSettings> {
    return LoopalDefaultSettingsSchema.parse(await this.call(
      input.workspaceId, 'desktop/updateSettings', input, token,
    ))
  }

  private async call(
    workspaceId: string,
    method: string,
    input: unknown,
    token: CancellationToken,
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

export function bindLoopalSettings(service: LoopalSettingsService): LoopalSettingsOperations {
  return {
    getLoopalSettings: service.getLoopalSettings.bind(service),
    updateLoopalSettings: service.updateLoopalSettings.bind(service),
  }
}
