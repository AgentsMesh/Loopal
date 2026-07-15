import {
  type RuntimeSummary,
  type SessionDetail,
  type Workspace,
} from '../../../../shared/contracts'
import { fakeRichAgents, fakeRichConversation, fakeSessionView } from './fake-rich-session'

export interface FakeSessionCatalog {
  readonly workspace: Workspace
  readonly details: Map<string, SessionDetail>
  readonly runtimes: Map<string, RuntimeSummary>
}

export function createFakeSessionCatalog(now: string): FakeSessionCatalog {
  const workspace: Workspace = {
    id: 'workspace-loopal',
    name: 'Loopal',
    rootUri: 'file:///workspace/loopal',
    kind: 'git_worktree',
  }
  const runtimeDesktop = runtime(
    'runtime-desktop-1', 'session-desktop', workspace.id, 'agent-root', now,
  )
  const runtimeProtocol = runtime(
    'runtime-protocol-1', 'session-protocol', workspace.id, 'agent-protocol', now,
  )
  const details = new Map<string, SessionDetail>([
    ['session-desktop', {
      session: {
        id: 'session-desktop', workspaceId: workspace.id,
        title: 'Build LoopalDesktop foundation', model: 'gpt-5', mode: 'agent',
        status: 'running', createdAt: now, updatedAt: now,
        activeRuntimeId: runtimeDesktop.id,
      },
      conversation: [
        entry('desktop-user', 'user', 'Build a durable desktop workbench around Loopal.', now),
        ...fakeRichConversation(now),
      ],
      agents: fakeRichAgents(),
      artifacts: [],
      view: fakeSessionView(now),
    }],
    ['session-protocol', {
      session: {
        id: 'session-protocol', workspaceId: workspace.id,
        title: 'Version Desktop Control Protocol', model: 'gpt-5', mode: 'agent',
        status: 'waiting', attention: 'permission', createdAt: now, updatedAt: now,
        activeRuntimeId: runtimeProtocol.id,
      },
      conversation: [entry(
        'protocol-system', 'system',
        'Waiting for permission to start the protocol compatibility check.', now,
      )],
      agents: [{ id: 'agent-protocol', name: 'Protocol agent', status: 'waiting' }],
      artifacts: [],
    }],
    ['session-audit', {
      session: {
        id: 'session-audit', workspaceId: workspace.id,
        title: 'Audit reference applications', model: 'gpt-5', mode: 'agent',
        status: 'stopped', attention: 'completed', createdAt: now, updatedAt: now,
      },
      conversation: [entry(
        'audit-assistant', 'assistant',
        'AgentsMesh and Synapse architecture audit complete.', now,
      )],
      agents: [{ id: 'agent-audit', name: 'Architecture agent', status: 'completed' }],
      artifacts: [{
        id: 'artifact-audit', sessionId: 'session-audit',
        title: 'Architecture findings.md', kind: 'document',
        uri: 'loopal-artifact://session-audit/findings.md', mediaType: 'text/markdown',
        producerAgentId: 'agent-audit', createdAt: now,
      }],
    }],
  ])
  return {
    workspace,
    details,
    runtimes: new Map([[runtimeDesktop.id, runtimeDesktop], [runtimeProtocol.id, runtimeProtocol]]),
  }
}

function runtime(
  id: string,
  sessionId: string,
  workspaceId: string,
  rootAgent: string,
  startedAt: string,
): RuntimeSummary {
  return {
    id, sessionId, workspaceId, generation: 1,
    state: 'ready', rootAgent, startedAt,
  }
}

function entry(
  id: string,
  role: 'user' | 'assistant' | 'system',
  text: string,
  createdAt: string,
) {
  return { id, role, text, createdAt }
}
