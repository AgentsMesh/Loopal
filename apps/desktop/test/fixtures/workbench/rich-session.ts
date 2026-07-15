import {
  type AgentSummary,
  type SessionDetail,
  type SessionView,
} from '../../../src/shared/contracts'
import {
  fakeRichAgents,
  fakeRichConversation,
  fakeSessionView,
} from '../../../src/platform/loopal-backend/node/fake/fake-rich-session'

export const richTimestamp = '2026-07-11T12:00:00.000Z'

export function richDetail(overrides: Partial<SessionDetail> = {}): SessionDetail {
  return {
    session: {
      id: 'session-rich', workspaceId: 'workspace', title: 'Rich session',
      model: 'gpt-5', mode: 'agent', status: 'running',
      createdAt: richTimestamp, updatedAt: richTimestamp, activeRuntimeId: 'runtime-rich',
    },
    conversation: fakeRichConversation(richTimestamp),
    agents: fakeRichAgents(),
    artifacts: [],
    view: fakeSessionView(richTimestamp),
    ...overrides,
  }
}

export function richView(overrides: Partial<SessionView> = {}): SessionView {
  return { ...fakeSessionView(richTimestamp), ...overrides }
}

export function richAgent(overrides: Partial<AgentSummary> = {}): AgentSummary {
  return {
    id: 'main', name: 'Loopal', status: 'running', model: 'gpt-5', mode: 'act',
    telemetry: {
      turnCount: 1, inputTokens: 100, outputTokens: 50,
      cacheCreationTokens: 20, cacheReadTokens: 30, thinkingTokens: 12,
      contextWindow: 1_000, toolsInFlight: 0, toolCount: 2,
    },
    ...overrides,
  }
}
