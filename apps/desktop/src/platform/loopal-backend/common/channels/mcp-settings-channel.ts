import { type CancellationToken } from '../../../../base/common/cancellation'
import {
  DeleteMcpServerInputSchema,
  ListMcpServersInputSchema,
  McpServersResponseSchema,
  UpsertMcpServerInputSchema,
} from '../../../../shared/contracts'
import { type DesktopBackend } from '../backend'

export async function callMcpSettingsBackend(
  backend: DesktopBackend,
  command: string,
  arg: unknown,
  token: CancellationToken,
): Promise<{ handled: false } | { handled: true; value: unknown }> {
  switch (command) {
    case 'listMcpServers': {
      const { workspaceId } = ListMcpServersInputSchema.parse(arg)
      const value = await backend.listMcpServers(workspaceId, token)
      return { handled: true, value: McpServersResponseSchema.parse(value) }
    }
    case 'upsertMcpServer': {
      const input = UpsertMcpServerInputSchema.parse(arg)
      const value = await backend.upsertMcpServer(input, token)
      return { handled: true, value: McpServersResponseSchema.parse(value) }
    }
    case 'deleteMcpServer': {
      const input = DeleteMcpServerInputSchema.parse(arg)
      const value = await backend.deleteMcpServer(input, token)
      return { handled: true, value: McpServersResponseSchema.parse(value) }
    }
    default:
      return { handled: false }
  }
}
