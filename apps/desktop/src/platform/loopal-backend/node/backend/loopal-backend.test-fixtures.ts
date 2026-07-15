import { vi } from 'vitest'
import { Emitter } from '../../../../base/common/event'
import { type HostStatus } from '../../../../shared/contracts'
import {
  type DesktopHostActivation, type DesktopHostSession,
} from '../../../desktop-host/node/host/desktop-host'
import { type JsonRpcNotification } from '../../../desktop-host/node/rpc/jsonrpc-client'
import { type DesktopHostClient, LoopalDesktopBackend } from './loopal-backend'
import { SessionRuntimeRegistry } from '../runtime/session-runtime-registry'
import { type SessionDirectoryRequest } from '../sessions/session-directory-authority'
import { defaultTestWorkspacePath } from './loopal-backend.test-paths'
export const timestamp = new Date('2026-07-11T12:00:00.000Z')
interface CatalogRow {
  id: string
  title: string
  model: string
  mode: string
  createdAt: string
  updatedAt: string
}
export class FakeHost implements DesktopHostClient {
  private readonly statuses = new Emitter<HostStatus>()
  private readonly notifications = new Emitter<JsonRpcNotification>()
  currentStatus: HostStatus = 'stopped'
  snapshotStatus = 'WaitingForInput'
  snapshotRole = 'assistant'
  snapshotContent: string
  snapshotStreaming = ''
  snapshotMessageId: string | undefined = 'message-1'
  snapshotRevision = 2
  emitSessionCreated = false
  failAfterSessionCreated?: Error
  readonly start = vi.fn(async (activate?: DesktopHostActivation): Promise<DesktopHostSession> => {
    this.status('spawning')
    const session = { sessionId: this.sessionId, serverVersion: 'test', pid: 42 }
    if (this.emitSessionCreated) {
      await activate?.(session)
      if (this.failAfterSessionCreated) throw this.failAfterSessionCreated
    }
    this.status('ready')
    this.ensureCatalogRow()
    return session
  })
  readonly stop = vi.fn(async () => {
    this.status('stopping')
    this.status('stopped')
  })
  readonly dispose = vi.fn()
  readonly request = vi.fn<DesktopHostClient['request']>(async (method, params) => {
    const input = params as Record<string, unknown> | undefined
    if (method === 'desktop/listSessions') return [...this.catalog]
    if (method === 'desktop/getSettings') return loopalSettings(input)
    if (method === 'desktop/updateSettings') return {
      ...loopalSettings(input), settings: input?.settings,
    }
    if (method === 'view/snapshot') return this.snapshot(String(input?.agent ?? 'main'))
    if (method === 'hub/topology') {
      return { agents: [
        { name: 'main', parent: null, children: ['worker'], lifecycle: 'running', model: 'model' },
        { name: 'worker', parent: 'main', children: [], lifecycle: 'running', model: 'model' },
      ] }
    }
    if (method === 'hub/list_agents') {
      return { agents: [{ name: 'main', state: 'connected' }, { name: 'worker', state: 'connected' }] }
    }
    if (method === 'hub/control') return { status: 'applied' }
    if (method === 'hub/status') return { agent_count: 2, uplink: null }
    if (method === 'workspace/gitStatus') {
      return { branch: 'main', ahead: 0, behind: 0, changes: [] }
    }
    if (method === 'workspace/listDirectory') {
      return { workspaceId: input?.workspaceId, path: input?.path, entries: [] }
    }
    return { ok: true }
  })
  readonly onStatus = this.statuses.event
  readonly onNotification = this.notifications.event
  constructor(readonly sessionId: string, private readonly catalog: CatalogRow[]) {
    this.snapshotContent = `Answer from ${sessionId}`
  }
  status(value: HostStatus): void {
    this.currentStatus = value
    this.statuses.fire(value)
  }
  crash(): void { this.status('crashed') }
  notification(method: string, params: unknown): void {
    this.notifications.fire({ method, params })
  }
  private ensureCatalogRow(): void {
    if (this.catalog.some((row) => row.id === this.sessionId)) return
    this.catalog.push(catalogRow(this.sessionId, timestamp.toISOString()))
  }
  private snapshot(agentName: string) {
    return {
      rev: this.snapshotRevision,
      state: { agent: {
        name: agentName,
        parent: agentName === 'main' ? null : 'main',
        children: agentName === 'main' ? ['worker'] : [],
        observable: { status: this.snapshotStatus },
        conversation: {
          streaming_text: this.snapshotStreaming,
          messages: [{
            role: this.snapshotRole,
            content: this.snapshotContent,
            ...(this.snapshotMessageId ? { message_id: this.snapshotMessageId } : {}),
          }],
        },
      } },
    }
  }
}
export function createBackend(options: {
  cwd?: string
  maxLive?: number
  freshSessions?: string[]
  catalog?: CatalogRow[]
  now?: () => Date
  defaultClock?: boolean
  sessionStatePath?: string
  hostSetup?: (host: FakeHost, index: number) => void
  sessionDirectoryRequest?: SessionDirectoryRequest
} = {}) {
  const catalog = options.catalog ?? [
    catalogRow('session-1', '2026-07-11T11:00:00.000Z'),
    catalogRow('session-2', '2026-07-11T10:00:00.000Z'),
  ]
  const fresh = [...(options.freshSessions ?? ['session-1', 'session-3', 'session-4'])]
  const hosts: FakeHost[] = []
  const inputs: Array<{ cwd: string; resumeSessionId?: string }> = []
  let runtimeIndex = 0
  const registry = new SessionRuntimeRegistry({
    maxLive: options.maxLive ?? 4,
    createRuntimeId: () => `runtime-${++runtimeIndex}`,
    createHost: (input) => {
      inputs.push(input)
      const host = new FakeHost(input.resumeSessionId ?? fresh.shift() ?? 'fresh-session', catalog)
      hosts.push(host)
      options.hostSetup?.(host, hosts.length - 1)
      return host
    },
  })
  const backend = new LoopalDesktopBackend({
    binaryPath: '/bin/loopal',
    cwd: options.cwd ?? defaultTestWorkspacePath,
    parentPid: 7,
    runtimeRegistry: registry,
    ...(options.sessionStatePath ? { sessionStatePath: options.sessionStatePath } : {}),
    ...(options.defaultClock ? {} : { now: options.now ?? (() => timestamp) }),
    ...(options.sessionDirectoryRequest
      ? { sessionDirectoryRequest: options.sessionDirectoryRequest } : {}),
  })
  return { backend, registry, hosts, inputs, catalog }
}

export function agentEvent(payload: unknown, eventId = 10, revision?: number): unknown {
  return {
    agent_name: { hub: [], agent: 'main' },
    event_id: eventId,
    turn_id: 1,
    correlation_id: 2,
    ...(revision === undefined ? {} : { rev: revision }),
    payload,
  }
}

function catalogRow(id: string, updatedAt: string): CatalogRow {
  return {
    id, title: `Raw ${id}`, model: 'loopal-default', mode: 'agent',
    createdAt: '2026-07-11T09:00:00.000Z', updatedAt,
  }
}

function loopalSettings(input?: Record<string, unknown>) {
  return {
    workspaceId: input?.workspaceId ?? 'local-workspace',
    settings: {
      model: 'gpt-5', modelRouting: emptyRouting(),
      permissionMode: 'bypass', decisionMode: 'manual',
      sandboxPolicy: 'default_write', thinking: { type: 'auto' },
      maxContextTokens: 0, memoryEnabled: true, microcompactIdleMinutes: 60,
      telemetryEnabled: true, outputStyle: '',
    },
    configuredProviders: ['test-provider'],
    providers: emptyProviders(),
    openaiCompatible: [],
    resolvedEntries: [{ key: 'model', value: 'gpt-5' }],
    settingSources: ['project local overrides'],
  }
}
function emptyRouting() {
  return { default: '', summarization: '', classification: '', refine: '' }
}
function emptyProviders() {
  const empty = () => ({ enabled: false, baseUrl: '', apiKeyEnv: '', apiKeyConfigured: false })
  return { anthropic: empty(), openai: empty(), google: empty() }
}
