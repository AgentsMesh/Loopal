import { vi } from 'vitest'
import { Emitter } from '../../../src/base/common/event'
import { type DesktopBackend } from '../../../src/platform/loopal-backend/common/backend'
import { type DesktopEvent } from '../../../src/shared/contracts'

const now = '2026-07-11T12:00:00.000Z'

export function createBackendStub(overrides: Partial<DesktopBackend> = {}): DesktopBackend {
  const events = new Emitter<DesktopEvent>()
  const detail = (sessionId: string) => ({
    session: {
      id: sessionId, workspaceId: 'workspace', title: 'Session', model: 'gpt-5',
      mode: 'agent', status: 'running' as const, createdAt: now, updatedAt: now,
      activeRuntimeId: `runtime-${sessionId}`,
    },
    conversation: [], agents: [], artifacts: [],
  })
  return {
    onEvent: events.event,
    bootstrap: vi.fn(async () => ({
      protocolVersion: 2 as const, hostStatus: 'ready' as const,
      workspaces: [], sessions: [], runtimes: [],
    })),
    openSession: vi.fn(async (sessionId: string) => detail(sessionId)),
    createSession: vi.fn(async () => detail('session-new')),
    stopSession: vi.fn(async () => undefined),
    restartSession: vi.fn(async (sessionId: string) => ({
      id: `runtime-${sessionId}`, sessionId, workspaceId: 'workspace', generation: 1,
      state: 'ready' as const, rootAgent: 'main', startedAt: now,
    })),
    sendMessage: vi.fn(async () => undefined),
    interruptAgent: vi.fn(async () => undefined),
    controlAgent: vi.fn(async () => ({ status: 'applied' as const })),
    getDesktopPreferences: vi.fn(async () => ({ locale: 'system' as const })),
    updateDesktopPreferences: vi.fn(async (input) => ({ ...input })),
    getLoopalSettings: vi.fn<DesktopBackend['getLoopalSettings']>(async (workspaceId) => ({
      workspaceId,
      settings: {
        model: 'gpt-5', modelRouting: emptyRouting(),
        permissionMode: 'bypass', decisionMode: 'manual',
        sandboxPolicy: 'default_write', thinking: { type: 'auto' as const },
        maxContextTokens: 0, memoryEnabled: true, microcompactIdleMinutes: 60,
        telemetryEnabled: true, outputStyle: '',
      },
      configuredProviders: [],
      ...settingsProjection(),
    })),
    updateLoopalSettings: vi.fn(async (input) => ({
      workspaceId: input.workspaceId, settings: input.settings,
      configuredProviders: [], ...settingsProjection(),
    })),
    listMcpServers: vi.fn(async (workspaceId) => ({ workspaceId, servers: [] })),
    upsertMcpServer: vi.fn(async (input) => ({ workspaceId: input.workspaceId, servers: [] })),
    deleteMcpServer: vi.fn(async (input) => ({ workspaceId: input.workspaceId, servers: [] })),
    listSkills: vi.fn(async (workspaceId) => ({ workspaceId, skills: [] })),
    getSkill: vi.fn(async ({ workspaceId, name }) => skillDetail(workspaceId, name)),
    upsertGlobalSkill: vi.fn(async (input) => skillDetail(
      input.workspaceId, input.name, input.description, input.body,
    )),
    deleteGlobalSkill: vi.fn(async (input) => ({ workspaceId: input.workspaceId, skills: [] })),
    listPlugins: vi.fn(async (workspaceId) => ({ workspaceId, plugins: [] })),
    getMetaHubSettings: vi.fn(async () => ({
      address: '', hubName: 'desktop-test', joinOnStart: false,
      startLocalOnLaunch: false, tokenConfigured: false,
    })),
    updateMetaHubSettings: vi.fn(async (input) => ({
      address: input.address, hubName: input.hubName,
      joinOnStart: input.joinOnStart, startLocalOnLaunch: input.startLocalOnLaunch,
      tokenConfigured: Boolean(input.token && !input.clearToken),
    })),
    getMetaHubStatus: vi.fn(async () => ({
      state: 'disconnected' as const, hubs: [], topology: [], refreshedAt: now,
    })),
    joinMetaHub: vi.fn(async () => ({
      state: 'connected' as const, hubs: [], topology: [], refreshedAt: now,
    })),
    disconnectMetaHub: vi.fn(async () => ({
      state: 'disconnected' as const, hubs: [], topology: [], refreshedAt: now,
    })),
    getLocalMetaHubStatus: vi.fn(async () => ({ state: 'stopped' as const })),
    startLocalMetaHub: vi.fn(async () => ({
      state: 'running' as const, address: '127.0.0.1:39000',
    })),
    stopLocalMetaHub: vi.fn(async () => ({ state: 'stopped' as const })),
    listDirectory: vi.fn(async ({ workspaceId, path }) => ({ workspaceId, path, entries: [] })),
    readFile: vi.fn(async ({ workspaceId, path }) => ({
      workspaceId, path, content: 'text', version: 'v1', languageId: 'plaintext', readonly: false,
    })),
    writeFile: vi.fn(async ({ workspaceId, path, content }) => ({
      workspaceId, path, content, version: 'v2', languageId: 'plaintext', readonly: false,
    })),
    searchWorkspace: vi.fn(async () => ({ matches: [], truncated: false })),
    gitStatus: vi.fn(async () => ({ branch: 'main', ahead: 0, behind: 0, changes: [] })),
    gitDiff: vi.fn(async ({ path }) => ({ path, patch: '', original: '', modified: '' })),
    gitStage: vi.fn(async () => undefined),
    gitUnstage: vi.fn(async () => undefined),
    listWorktrees: vi.fn(async () => []),
    createWorktree: vi.fn(async ({ name }) => ({
      id: name, path: `/tmp/${name}`, branch: `loopal-wt-${name}`, head: 'abc',
      isMain: false, hasChanges: false,
    })),
    removeWorktree: vi.fn(async () => undefined),
    respondPermission: vi.fn(async () => undefined),
    respondQuestion: vi.fn(async () => undefined),
    respondPlanApproval: vi.fn(async () => undefined),
    ...overrides,
  }
}

function skillDetail(
  workspaceId: string, name: string, description = 'Test skill', body = 'Test $ARGUMENTS',
) {
  return {
    workspaceId, name, description, body, hasArguments: body.includes('$ARGUMENTS'),
    source: 'global', scope: 'global' as const, editable: true, effective: true,
    revision: '1'.padStart(64, '0'),
  }
}

function emptyRouting() {
  return { default: '', summarization: '', classification: '', refine: '' }
}

function settingsProjection() {
  const empty = () => ({ enabled: false, baseUrl: '', apiKeyEnv: '', apiKeyConfigured: false })
  return {
    providers: { anthropic: empty(), openai: empty(), google: empty() },
    openaiCompatible: [],
    resolvedEntries: [{ key: 'model', value: 'gpt-5' }],
    settingSources: ['defaults'],
  }
}
