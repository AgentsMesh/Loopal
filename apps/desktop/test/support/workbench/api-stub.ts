import { Emitter } from '../../../src/base/common/event'
import {
  type DesktopEvent, type LoopalDesktopAPI, type SessionDetail,
  type SessionSummary, type WorkbenchBootstrap,
} from '../../../src/shared/contracts'

export const updatedAt = '2026-07-11T12:00:00.000Z'

export const sessionOne: SessionSummary = {
  id: 'session-1', workspaceId: 'workspace', title: 'Build the desktop workbench',
  model: 'gpt-5', mode: 'agent', status: 'running', createdAt: updatedAt,
  updatedAt, activeRuntimeId: 'runtime-1',
}

export const sessionTwo: SessionSummary = {
  id: 'session-2', workspaceId: 'workspace', title: 'Version the protocol',
  model: 'gpt-5', mode: 'agent', status: 'waiting', attention: 'permission',
  createdAt: updatedAt, updatedAt, activeRuntimeId: 'runtime-2',
}

export function sessionDetail(session: SessionSummary): SessionDetail {
  return {
    session,
    conversation: [{
      id: `message-${session.id}`, role: 'assistant',
      text: `Conversation for ${session.title}`, createdAt: updatedAt,
    }],
    agents: [{
      id: `agent-${session.id}`, name: 'Loopal Agent',
      status: session.id === sessionOne.id ? 'running' : 'waiting',
    }],
    artifacts: session.id === sessionTwo.id ? [{
      id: 'artifact-1', sessionId: session.id, title: 'Protocol.md', kind: 'document',
      uri: 'loopal-artifact://protocol', mediaType: 'text/markdown',
      producerAgentId: `agent-${session.id}`, createdAt: updatedAt,
    }] : [],
  }
}

export function createTestAPI(overrides: Partial<LoopalDesktopAPI> = {}) {
  const events = new Emitter<DesktopEvent>()
  const bootstrap: WorkbenchBootstrap = {
    protocolVersion: 2,
    hostStatus: 'ready',
    workspaces: [],
    sessions: [sessionOne, sessionTwo],
    runtimes: [
      {
        id: 'runtime-1', sessionId: sessionOne.id, workspaceId: 'workspace', generation: 1,
        state: 'ready', rootAgent: 'agent-session-1', startedAt: updatedAt,
      },
      {
        id: 'runtime-2', sessionId: sessionTwo.id, workspaceId: 'workspace', generation: 1,
        state: 'ready', rootAgent: 'agent-session-2', startedAt: updatedAt,
      },
    ],
    activeSessionId: sessionOne.id,
  }
  const api: LoopalDesktopAPI = {
    bootstrap: async () => bootstrap,
    openSession: async (sessionId) => sessionDetail(
      sessionId === sessionTwo.id ? sessionTwo : sessionOne,
    ),
    createSession: async () => sessionDetail(sessionOne),
    selectSessionDirectory: async () => undefined,
    stopSession: async () => undefined,
    restartSession: async (sessionId) => bootstrap.runtimes
      .find((runtime) => runtime.sessionId === sessionId)!,
    selectImages: async () => [],
    sendMessage: async () => undefined,
    interruptAgent: async () => undefined,
    controlAgent: async () => undefined,
    getDesktopPreferences: async () => ({ locale: 'system' }),
    updateDesktopPreferences: async (input) => ({ ...input }),
    getLoopalSettings: async (workspaceId) => ({
      workspaceId,
      settings: {
        model: 'gpt-5', modelRouting: emptyRouting(),
        permissionMode: 'bypass', decisionMode: 'manual',
        sandboxPolicy: 'default_write', thinking: { type: 'auto' },
        maxContextTokens: 0, memoryEnabled: true, microcompactIdleMinutes: 60,
        telemetryEnabled: true, outputStyle: '',
      },
      configuredProviders: [],
      ...settingsProjection(),
    }),
    updateLoopalSettings: async (input) => ({
      workspaceId: input.workspaceId, settings: input.settings,
      configuredProviders: [], ...settingsProjection(),
    }),
    listMcpServers: async (workspaceId) => ({ workspaceId, servers: [] }),
    upsertMcpServer: async (input) => ({ workspaceId: input.workspaceId, servers: [] }),
    deleteMcpServer: async (input) => ({ workspaceId: input.workspaceId, servers: [] }),
    listSkills: async (workspaceId) => ({ workspaceId, skills: [] }),
    getSkill: async ({ workspaceId, name }) => ({
      workspaceId, name, description: 'Test skill', body: 'Test $ARGUMENTS',
      hasArguments: true, source: 'global', scope: 'global', editable: true, effective: true,
      revision: '1'.padStart(64, '0'),
    }),
    upsertGlobalSkill: async (input) => ({
      workspaceId: input.workspaceId, name: input.name, description: input.description,
      body: input.body, hasArguments: input.body.includes('$ARGUMENTS'),
      source: 'global', scope: 'global', editable: true, effective: true,
      revision: '2'.padStart(64, '0'),
    }),
    deleteGlobalSkill: async (input) => ({ workspaceId: input.workspaceId, skills: [] }),
    listPlugins: async (workspaceId) => ({ workspaceId, plugins: [] }),
    getMetaHubSettings: async () => ({
      address: '', hubName: 'desktop-test', joinOnStart: false,
      startLocalOnLaunch: false, tokenConfigured: false,
    }),
    updateMetaHubSettings: async (input) => ({
      address: input.address, hubName: input.hubName,
      joinOnStart: input.joinOnStart, startLocalOnLaunch: input.startLocalOnLaunch,
      tokenConfigured: Boolean(input.token && !input.clearToken),
    }),
    getMetaHubStatus: async () => ({
      state: 'disconnected', hubs: [], topology: [], refreshedAt: updatedAt,
    }),
    joinMetaHub: async (input) => ({
      state: 'connected', address: input.address, hubName: input.hubName,
      hubs: [], topology: [], refreshedAt: updatedAt,
    }),
    disconnectMetaHub: async () => ({
      state: 'disconnected', hubs: [], topology: [], refreshedAt: updatedAt,
    }),
    getLocalMetaHubStatus: async () => ({ state: 'stopped' }),
    startLocalMetaHub: async () => ({ state: 'running', address: '127.0.0.1:39000' }),
    stopLocalMetaHub: async () => ({ state: 'stopped' }),
    listDirectory: async ({ workspaceId, path }) => ({ workspaceId, path, entries: [] }),
    readFile: async ({ workspaceId, path }) => ({
      workspaceId, path, content: '', version: 'test-1', languageId: 'plaintext', readonly: false,
    }),
    writeFile: async ({ workspaceId, path, content }) => ({
      workspaceId, path, content, version: 'test-2', languageId: 'plaintext', readonly: false,
    }),
    searchWorkspace: async () => ({ matches: [], truncated: false }),
    gitStatus: async () => ({ branch: 'main', ahead: 0, behind: 0, changes: [] }),
    gitDiff: async ({ path }) => ({ path, patch: '', original: '', modified: '' }),
    gitStage: async () => undefined,
    gitUnstage: async () => undefined,
    listWorktrees: async () => [],
    createWorktree: async ({ name }) => ({
      id: name, path: `/tmp/${name}`, branch: `loopal-wt-${name}`, head: 'abc',
      isMain: false, hasChanges: false,
    }),
    removeWorktree: async () => undefined,
    respondPermission: async () => undefined,
    respondQuestion: async () => undefined,
    respondPlanApproval: async () => undefined,
    onEvent: (listener) => {
      const subscription = events.event(listener)
      return () => subscription.dispose()
    },
    ...overrides,
  }
  return { api, events }
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
