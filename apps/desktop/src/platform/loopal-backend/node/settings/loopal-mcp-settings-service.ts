import { type CancellationToken, throwIfCancelled } from '../../../../base/common/cancellation'
import {
  McpServersResponseSchema,
  type DeleteMcpServerInput,
  type McpServersResponse,
  type UpsertMcpServerInput,
} from '../../../../shared/contracts'
import { type DesktopBackend } from '../../common/backend'
import { type CodeWorkbenchRuntimeRouter } from '../workspace/loopal-code-workbench'

export type LoopalMcpSettingsOperations = Pick<
  DesktopBackend, 'listMcpServers' | 'upsertMcpServer' | 'deleteMcpServer'
>

export class LoopalMcpSettingsService implements LoopalMcpSettingsOperations {
  constructor(private readonly router: CodeWorkbenchRuntimeRouter) {}

  async listMcpServers(
    workspaceId: string, token: CancellationToken,
  ): Promise<McpServersResponse> {
    return this.call(workspaceId, 'desktop/listMcpServers', { workspaceId }, token)
  }

  async upsertMcpServer(
    input: UpsertMcpServerInput, token: CancellationToken,
  ): Promise<McpServersResponse> {
    return this.call(input.workspaceId, 'desktop/upsertMcpServer', input, token)
  }

  async deleteMcpServer(
    input: DeleteMcpServerInput, token: CancellationToken,
  ): Promise<McpServersResponse> {
    return this.call(input.workspaceId, 'desktop/deleteMcpServer', input, token)
  }

  private async call(
    workspaceId: string, method: string, input: unknown, token: CancellationToken,
  ): Promise<McpServersResponse> {
    throwIfCancelled(token)
    const runtime = await this.router.workspace(workspaceId)
    throwIfCancelled(token)
    const controller = new AbortController()
    const subscription = token.onCancellationRequested(() => controller.abort())
    try {
      const result = await runtime.host.request(method, input, controller.signal)
      throwIfCancelled(token)
      return McpServersResponseSchema.parse(result)
    } finally {
      subscription.dispose()
    }
  }
}

export function bindLoopalMcpSettings(
  service: LoopalMcpSettingsService,
): LoopalMcpSettingsOperations {
  return {
    listMcpServers: service.listMcpServers.bind(service),
    upsertMcpServer: service.upsertMcpServer.bind(service),
    deleteMcpServer: service.deleteMcpServer.bind(service),
  }
}
