import {
  McpServerInputSchema,
  type McpSecretPatch,
  type McpSecretStatus,
  type McpServerDefinition,
  type McpServerInput,
} from '../../../../shared/contracts'

export interface McpServerDraft {
  readonly lockedName: boolean
  readonly restrictedSecrets: boolean
  readonly type: 'stdio' | 'streamable-http'
  readonly name: string
  readonly enabled: boolean
  readonly timeoutMs: number
  readonly sharing: 'hub-singleton' | 'per-agent' | 'spawn-tree'
  readonly command: string
  readonly argsText: string
  readonly url: string
  readonly cwdIsolation: boolean
  readonly cwdArg: string
  readonly cacheSubdir: string
  readonly secrets: readonly McpSecretStatus[]
  readonly secretPatches: readonly McpSecretPatch[]
}

export function newMcpServerDraft(): McpServerDraft {
  return {
    lockedName: false, restrictedSecrets: false,
    type: 'stdio', name: '', enabled: true, timeoutMs: 30_000,
    sharing: 'hub-singleton', command: '', argsText: '', url: '', cwdIsolation: false,
    cwdArg: '--user-data-dir', cacheSubdir: '', secrets: [], secretPatches: [],
  }
}

export function editMcpServerDraft(server: McpServerDefinition): McpServerDraft {
  const common = {
    lockedName: true,
    restrictedSecrets: !['project', 'local'].includes(server.source)
      && (server.type === 'stdio' ? server.env : server.headers).some((entry) => entry.configured),
    type: server.type, name: server.name, enabled: server.enabled,
    timeoutMs: server.timeoutMs, sharing: server.sharing, secretPatches: [],
  } as const
  if (server.type === 'stdio') return {
    ...common, command: server.command, argsText: server.args.join('\n'), url: '',
    cwdIsolation: server.cwdIsolation !== null,
    cwdArg: server.cwdIsolation?.arg ?? '--user-data-dir',
    cacheSubdir: server.cwdIsolation?.cacheSubdir ?? '', secrets: server.env,
  }
  return {
    ...common, command: '', argsText: '', url: server.url, cwdIsolation: false,
    cwdArg: '--user-data-dir', cacheSubdir: '', secrets: server.headers,
  }
}

export function mcpInputFromDraft(draft: McpServerDraft): McpServerInput {
  const common = {
    type: draft.type, name: draft.name, enabled: draft.enabled,
    timeoutMs: draft.timeoutMs, sharing: draft.sharing,
    secretPatches: draft.secretPatches,
  }
  if (draft.type === 'stdio') return McpServerInputSchema.parse({
    ...common, command: draft.command,
    args: draft.argsText === '' ? [] : draft.argsText.split('\n'),
    cwdIsolation: draft.cwdIsolation ? {
      arg: draft.cwdArg, ...(draft.cacheSubdir ? { cacheSubdir: draft.cacheSubdir } : {}),
    } : null,
  })
  return McpServerInputSchema.parse({ ...common, url: draft.url })
}

export function withSecretPatch(
  draft: McpServerDraft, patch: McpSecretPatch | undefined, name: string,
): McpServerDraft {
  return {
    ...draft,
    secretPatches: [
      ...draft.secretPatches.filter((candidate) => candidate.name !== name),
      ...(patch ? [patch] : []),
    ],
  }
}
