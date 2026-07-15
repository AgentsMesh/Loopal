import { CancellationToken, throwIfCancelled } from '../../../../base/common/cancellation'
import {
  DeleteMcpServerInputSchema,
  UpsertMcpServerInputSchema,
  type McpServerDefinition,
  type McpServersResponse,
} from '../../../../shared/contracts'
import { type LoopalMcpSettingsOperations } from '../settings/loopal-mcp-settings-service'

export function bindFakeMcpSettings(workspaceId: string): LoopalMcpSettingsOperations {
  let servers: McpServerDefinition[] = []
  const response = (): McpServersResponse => ({ workspaceId, servers: structuredClone(servers) })
  const requireWorkspace = (input: string): void => {
    if (input !== workspaceId) throw new Error(`Unknown workspace: ${input}`)
  }
  return {
    listMcpServers: async (input, token = CancellationToken.None) => {
      throwIfCancelled(token)
      requireWorkspace(input)
      return response()
    },
    upsertMcpServer: async (input, token = CancellationToken.None) => {
      throwIfCancelled(token)
      const parsed = UpsertMcpServerInputSchema.parse(input)
      requireWorkspace(parsed.workspaceId)
      const previous = servers.find((server) => server.name === parsed.server.name)
      const statuses = applySecretPatches(previous, parsed.server.secretPatches)
      let next: McpServerDefinition
      if (parsed.server.type === 'stdio') {
        const { secretPatches: _secretPatches, ...definition } = parsed.server
        next = { ...definition, source: 'local', env: statuses }
      } else {
        const { secretPatches: _secretPatches, ...definition } = parsed.server
        next = { ...definition, source: 'local', headers: statuses }
      }
      servers = [...servers.filter((server) => server.name !== next.name), next]
      return response()
    },
    deleteMcpServer: async (input, token = CancellationToken.None) => {
      throwIfCancelled(token)
      const parsed = DeleteMcpServerInputSchema.parse(input)
      requireWorkspace(parsed.workspaceId)
      servers = servers.filter((server) => server.name !== parsed.name)
      return response()
    },
  }
}

export function bindUnavailableMcpSettings(reason: string): LoopalMcpSettingsOperations {
  const fail = (token: CancellationToken): never => {
    throwIfCancelled(token)
    throw new Error(reason)
  }
  return {
    listMcpServers: async (_workspaceId, token) => fail(token),
    upsertMcpServer: async (_input, token) => fail(token),
    deleteMcpServer: async (_input, token) => fail(token),
  }
}

function applySecretPatches(
  previous: McpServerDefinition | undefined,
  patches: readonly { name: string; operation: 'set' | 'remove' }[],
) {
  const current = previous?.type === 'stdio' ? previous.env
    : previous?.type === 'streamable-http' ? previous.headers : []
  const names = new Set(current.filter((secret) => secret.configured).map((secret) => secret.name))
  for (const patch of patches) {
    if (patch.operation === 'set') names.add(patch.name)
    else names.delete(patch.name)
  }
  return [...names].sort().map((name) => ({ name, configured: true }))
}
