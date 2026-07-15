import {
  McpServersResponseSchema,
  type DeleteMcpServerInput,
  type McpServersResponse,
  type UpsertMcpServerInput,
} from '../../../../shared/contracts'
import { type ChannelClient } from '../../../ipc/common/channel'

export interface McpSettingsClientOperations {
  listMcpServers(workspaceId: string): Promise<McpServersResponse>
  upsertMcpServer(input: UpsertMcpServerInput): Promise<McpServersResponse>
  deleteMcpServer(input: DeleteMcpServerInput): Promise<McpServersResponse>
}

export function bindMcpSettingsClient(client: ChannelClient): McpSettingsClientOperations {
  return {
    listMcpServers: async (workspaceId) => McpServersResponseSchema.parse(
      await client.call('desktopBackend', 'listMcpServers', { workspaceId }),
    ),
    upsertMcpServer: async (input) => McpServersResponseSchema.parse(
      await client.call('desktopBackend', 'upsertMcpServer', input),
    ),
    deleteMcpServer: async (input) => McpServersResponseSchema.parse(
      await client.call('desktopBackend', 'deleteMcpServer', input),
    ),
  }
}
